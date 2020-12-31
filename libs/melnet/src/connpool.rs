use crate::common::*;
use crate::reqs::*;
use by_address::ByAddress;
use lazy_static::lazy_static;
use log::trace;
use min_max_heap::MinMaxHeap;
use parking_lot::RwLock;
use serde::{de::DeserializeOwned, Serialize};
use smol::Timer;
use smol::{channel::Receiver, prelude::*};
use smol::{channel::Sender, net::TcpStream};
use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};

lazy_static! {
    static ref CONN_POOL: Client = Client::default();
}

/// Returns a reference to the global ConnPool instance.
pub fn g_client() -> &'static Client {
    &CONN_POOL
}

/// Implements a thread-safe pool of connections to melnet, or any HTTP/1.1-style keepalive protocol, servers.
#[derive(Debug, Default)]
pub struct Client {
    pool: RwLock<HashMap<SocketAddr, SingleHost>>,
}

impl Client {
    /// Connects to a given address, which may return either a new connection or an existing one.
    pub async fn connect(&self, addr: impl ToSocketAddrs) -> std::io::Result<TcpStream> {
        let addr = addr.to_socket_addrs()?.next().unwrap();
        let existing = {
            let pool = self.pool.read();
            let existing = pool.get(&addr);
            match existing {
                Some(existing) => Some(existing.clone()),
                None => None,
            }
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
    pub fn recycle(&self, conn: TcpStream) {
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
    pub async fn request<TInput: Serialize, TOutput: DeserializeOwned>(
        &self,
        addr: SocketAddr,
        netname: &str,
        verb: &str,
        req: TInput,
    ) -> Result<TOutput> {
        // grab a connection
        let mut conn = self.connect(addr).await.map_err(MelnetError::Network)?;
        // send a request
        let rr = bincode::serialize(&RawRequest {
            proto_ver: PROTO_VER,
            netname: netname.to_owned(),
            verb: verb.to_owned(),
            payload: bincode::serialize(&req).unwrap(),
        })
        .unwrap();
        write_len_bts(&mut conn, &rr).await?;
        // read the response length
        let response: RawResponse =
            bincode::deserialize(&read_len_bts(&mut conn).await?).map_err(|e| {
                MelnetError::Network(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            })?;
        let response = match response.kind.as_ref() {
            "Ok" => bincode::deserialize::<TOutput>(&response.body)
                .map_err(|_| MelnetError::Custom("bincode error".to_owned()))?,
            "NoVerb" => return Err(MelnetError::VerbNotFound),
            _ => {
                return Err(MelnetError::Custom(
                    String::from_utf8_lossy(&response.body).to_string(),
                ))
            }
        };
        // put the connection back
        self.recycle(conn);
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
        let deadline = async {
            if let Some((min, _)) = heap.peek_min() {
                let now = Instant::now();
                if now < *min {
                    Timer::after(*min - now).await;
                } else {
                    Timer::after(Duration::from_secs(0)).await;
                }
            } else {
                smol::future::pending().await
            };
        };

        let evt: Evt = async { Some(Evt::Insertion(recv_insertion.recv().await.ok()?)) }
            .or(async { Some(Evt::Request(recv_request.recv().await.ok()?)) })
            .or(async {
                deadline.await;
                Some(Evt::Timeout)
            })
            .await?;

        match evt {
            Evt::Insertion(insertion) => {
                let inserted_deadline = Instant::now() + Duration::from_secs(1);
                heap.push((inserted_deadline, ByAddress(Box::new(insertion))));
            }
            Evt::Request(send_response) => send_response
                .send(match heap.pop_max() {
                    Some(max) => {
                        let ByAddress(bx) = max.1;
                        Some(*bx)
                    }
                    None => None,
                })
                .await
                .unwrap(),
            Evt::Timeout => {
                heap.pop_min();
            }
        }
    }
}
