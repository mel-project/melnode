use crate::common::*;
use crate::reqs::*;
use by_address::ByAddress;
use lazy_static::lazy_static;
use log::trace;
use min_max_heap::MinMaxHeap;
use parking_lot::RwLock;
use serde::{de::DeserializeOwned, Serialize};
use smol::{channel::Receiver, prelude::*};
use smol::{channel::Sender, net::TcpStream};
use smol::{lock::Semaphore, Timer};
use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};

lazy_static! {
    static ref CONN_POOL: Client = Client::default();
}

/// Does a melnet request to any given endpoint, using the global client.
pub async fn request<TInput: Serialize, TOutput: DeserializeOwned + std::fmt::Debug>(
    addr: SocketAddr,
    netname: &str,
    verb: &str,
    req: TInput,
) -> Result<TOutput> {
    CONN_POOL.request(addr, netname, verb, req).await
}

/// Implements a thread-safe pool of connections to melnet, or any HTTP/1.1-style keepalive protocol, servers.
#[derive(Debug, Default)]
pub struct Client {
    pool: RwLock<HashMap<SocketAddr, SingleHost>>,
}

impl Client {
    /// Connects to a given address, which may return either a new connection or an existing one.
    async fn connect(&self, addr: impl ToSocketAddrs) -> std::io::Result<TcpStream> {
        let addr = addr.to_socket_addrs()?.next().unwrap();
        let existing = {
            let pool = self.pool.read();
            let existing = pool.get(&addr);
            existing.cloned()
        };
        match existing {
            Some(existing) => {
                let existing = existing.clone();
                match existing.get_conn().await {
                    Some(conn) => Ok(conn),
                    None => {
                        trace!("connect({:?}) -> fresh", addr);
                        TcpStream::connect(addr).await
                    }
                }
            }
            None => {
                // create a new connection
                trace!("connect({:?}) -> fresh", addr);
                TcpStream::connect(addr).await
            }
        }
    }
    /// Takes ownership of and returns a given TCP connection back to the pool.
    fn recycle(&self, conn: TcpStream) {
        let addr = conn.peer_addr().unwrap();
        self.pool
            .write()
            .entry(addr)
            .or_insert_with(SingleHost::new)
            .send_insertion
            .try_send(conn)
            .unwrap();
    }
    /// Does a melnet request to any given endpoint.
    pub async fn request<TInput: Serialize, TOutput: DeserializeOwned + std::fmt::Debug>(
        &self,
        addr: SocketAddr,
        netname: &str,
        verb: &str,
        req: TInput,
    ) -> Result<TOutput> {
        // Semaphore
        static GLOBAL_LIMIT: Semaphore = Semaphore::new(128);
        let _guard = GLOBAL_LIMIT.acquire().await;
        let start = Instant::now();
        // grab a connection
        let mut conn = self.connect(addr).await.map_err(MelnetError::Network)?;
        conn.set_nodelay(true).unwrap();
        // send a request
        let rr = stdcode::serialize(&RawRequest {
            proto_ver: PROTO_VER,
            netname: netname.to_owned(),
            verb: verb.to_owned(),
            payload: stdcode::serialize(&req).unwrap(),
        })
        .unwrap();
        write_len_bts(&mut conn, &rr).await?;
        // read the response length
        let response: RawResponse =
            stdcode::deserialize(&read_len_bts(&mut conn).await?).map_err(|e| {
                MelnetError::Network(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            })?;
        let response = match response.kind.as_ref() {
            "Ok" => stdcode::deserialize::<TOutput>(&response.body)
                .map_err(|_| MelnetError::Custom("stdcode error".to_owned()))?,
            "NoVerb" => return Err(MelnetError::VerbNotFound),
            _ => {
                return Err(MelnetError::Custom(
                    String::from_utf8_lossy(&response.body).to_string(),
                ))
            }
        };
        // put the connection back
        self.recycle(conn);
        let elapsed = start.elapsed();
        if elapsed.as_secs_f64() > 3.0 {
            log::warn!("melnet req to {} took {:?}", addr, elapsed)
        }
        Ok(response)
    }
}

#[derive(Debug, Clone)]
struct SingleHost {
    send_insertion: Sender<TcpStream>,
    send_request: Sender<Sender<Option<TcpStream>>>,
}

impl SingleHost {
    fn new() -> Self {
        let (send_insertion, recv_insertion) = smol::channel::unbounded();
        let (send_request, recv_request) = smol::channel::unbounded();
        smolscale::spawn(async {
            singlehost_monitor(recv_insertion, recv_request).await;
        })
        .detach();
        SingleHost {
            send_insertion,
            send_request,
        }
    }
    async fn get_conn(&self) -> Option<TcpStream> {
        let (send, recv) = smol::channel::unbounded();
        self.send_request.send(send).await.unwrap();
        recv.recv().await.unwrap()
    }
}

async fn singlehost_monitor(
    recv_insertion: Receiver<TcpStream>,
    recv_request: Receiver<Sender<Option<TcpStream>>>,
) -> Option<()> {
    let mut heap: MinMaxHeap<(Instant, ByAddress<Box<TcpStream>>)> = MinMaxHeap::new();

    enum Evt {
        Insertion(TcpStream),
        Request(Sender<Option<TcpStream>>),
        Timeout,
    }

    loop {
        let heap_overflow = heap.len() > 256;
        let deadline = async {
            if heap_overflow {
            } else if let Some((min, _)) = heap.peek_min() {
                Timer::at(*min).await;
            } else {
                smol::future::pending().await
            };
        };

        let evt: Evt = async {
            deadline.await;
            Some(Evt::Timeout)
        }
        .or(async { Some(Evt::Insertion(recv_insertion.recv().await.ok()?)) })
        .or(async { Some(Evt::Request(recv_request.recv().await.ok()?)) })
        .await?;

        match evt {
            Evt::Insertion(insertion) => {
                let inserted_deadline = Instant::now() + Duration::from_secs(60);
                heap.push((inserted_deadline, ByAddress(Box::new(insertion))));
            }
            Evt::Request(send_response) => {
                let _ = send_response
                    .send(match heap.pop_max() {
                        Some(max) => {
                            let ByAddress(bx) = max.1;
                            Some(*bx)
                        }
                        None => None,
                    })
                    .await;
            }
            Evt::Timeout => {
                heap.pop_min();
            }
        }
    }
}
