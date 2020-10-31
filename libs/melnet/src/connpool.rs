use crate::common::*;
use crate::reqs::*;
use async_net::TcpStream;
use by_address::ByAddress;
use futures::channel::mpsc;
use futures::channel::mpsc::unbounded;
use futures::channel::oneshot;
use futures::prelude::*;
use futures::select;
use lazy_static::lazy_static;
use log::trace;
use min_max_heap::MinMaxHeap;
use parking_lot::RwLock;
use serde::{de::DeserializeOwned, Serialize};
use smol::Timer;
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
            .unbounded_send(conn)
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
                .map_err(|_| MelnetError::Custom("rlp error".to_owned()))?,
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
    send_insertion: mpsc::UnboundedSender<TcpStream>,
    send_request: mpsc::UnboundedSender<oneshot::Sender<Option<TcpStream>>>,
}

impl SingleHost {
    fn new() -> Self {
        let (send_insertion, recv_insertion) = unbounded();
        let (send_request, recv_request) = unbounded();
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
        let (send, recv) = oneshot::channel();
        self.send_request.unbounded_send(send).unwrap();
        recv.await.unwrap()
    }
}

async fn singlehost_monitor(
    mut recv_insertion: mpsc::UnboundedReceiver<TcpStream>,
    mut recv_request: mpsc::UnboundedReceiver<oneshot::Sender<Option<TcpStream>>>,
) {
    let mut heap: MinMaxHeap<(Instant, ByAddress<Box<TcpStream>>)> = MinMaxHeap::new();
    loop {
        let deadline = if let Some((min, _)) = heap.peek_min() {
            let now = Instant::now();
            if now < *min {
                Timer::after(*min - now)
            } else {
                Timer::after(Duration::from_secs(0))
            }
        } else {
            Timer::after(Duration::from_secs(86400))
        };

        select! {
            insertion = recv_insertion.next() => {
                if let Some(insertion) = insertion {
                    let inserted_deadline = Instant::now() + Duration::from_secs(1);
                    heap.push((inserted_deadline, ByAddress(Box::new(insertion))));
                }
            }
            send_response = recv_request.next() => {
                if let Some(send_response) = send_response {
                    send_response.send(match heap.pop_max() {
                        Some(max) => {
                            let ByAddress(bx) = max.1;
                            Some(*bx)
                        }
                        None => None
                    }).unwrap()
                } else {
                    return
                }
            }
            _ = deadline.fuse() => {
                heap.pop_min(); // this drops the tcp connection too!
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    #[test]
    fn simple() {
        let _ = env_logger::try_init();
        smol::run(async {
            // spawn a stupid echo server that only listens once
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            Task::blocking(async move {
                let (mut cconn, _) = listener.accept().unwrap();
                std::io::copy(&mut cconn.try_clone().unwrap(), &mut cconn).unwrap();
            })
            .detach();
            println!("done here");
            // connect 5 times; echo must work all the times
            let pool = Client::default();
            for count in 0..5 {
                let test_str = format!("echo-{}", count);
                println!("wait for connect");
                let mut conn = pool.connect(addr).await.unwrap();
                conn.write_all(&test_str.clone().into_bytes())
                    .await
                    .unwrap();
                let mut buf = [0; 6];
                conn.read_exact(&mut buf).await.unwrap();
                assert_eq!(buf.to_vec(), test_str.into_bytes());
                pool.recycle(conn);
                Timer::after(Duration::from_millis(10)).await;
            }
        })
    }
}
