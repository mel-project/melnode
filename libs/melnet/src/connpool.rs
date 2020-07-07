use by_address::ByAddress;
use futures::channel::mpsc;
use futures::channel::mpsc::unbounded;
use futures::channel::oneshot;
use futures::prelude::*;
use futures::select;
use log::trace;
use min_max_heap::MinMaxHeap;
use parking_lot::RwLock;
use smol::*;
use std::collections::HashMap;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// Implements a thread-safe pool of connections to melnet, or any HTTP/1.1-style keepalive protocol, servers.
#[derive(Debug, Default)]
pub struct ConnPool {
    pool: RwLock<HashMap<SocketAddr, SingleHost>>,
}

impl ConnPool {
    // Connects to a given address, which may return either a new connection or an existing one.
    pub async fn connect(&self, addr: impl ToSocketAddrs) -> std::io::Result<Async<TcpStream>> {
        let addr = addr.to_socket_addrs()?.next().unwrap();
        let pool = self.pool.read();
        let existing = pool.get(&addr);
        match existing {
            Some(existing) => match existing.get_conn().await {
                Some(conn) => Ok(conn),
                None => {
                    trace!("connect({:?}) -> fresh", addr);
                    Async::<TcpStream>::connect(&addr).await
                }
            },
            None => {
                // create a new connection
                trace!("connect({:?}) -> fresh", addr);
                Async::<TcpStream>::connect(&addr).await
            }
        }
    }
    // Takes ownership of and returns a given TCP connection back to the pool.
    pub fn recycle(&self, conn: Async<TcpStream>) {
        let addr = conn.get_ref().peer_addr().unwrap();
        trace!("recycle({:?})", addr);
        self.pool
            .write()
            .entry(addr)
            .or_insert_with(SingleHost::new)
            .send_insertion
            .unbounded_send(conn)
            .unwrap();
    }
}

#[derive(Debug)]
struct SingleHost {
    send_insertion: mpsc::UnboundedSender<Async<TcpStream>>,
    send_request: mpsc::UnboundedSender<oneshot::Sender<Option<Async<TcpStream>>>>,
}

impl SingleHost {
    fn new() -> Self {
        let (send_insertion, recv_insertion) = unbounded();
        let (send_request, recv_request) = unbounded();
        Task::spawn(async {
            singlehost_monitor(recv_insertion, recv_request).await;
        })
        .detach();
        SingleHost {
            send_insertion,
            send_request,
        }
    }
    async fn get_conn(&self) -> Option<Async<TcpStream>> {
        let (send, recv) = oneshot::channel();
        self.send_request.unbounded_send(send).unwrap();
        recv.await.unwrap()
    }
}

async fn singlehost_monitor(
    mut recv_insertion: mpsc::UnboundedReceiver<Async<TcpStream>>,
    mut recv_request: mpsc::UnboundedReceiver<oneshot::Sender<Option<Async<TcpStream>>>>,
) {
    let mut heap: MinMaxHeap<(Instant, ByAddress<Box<Async<TcpStream>>>)> = MinMaxHeap::new();
    loop {
        let deadline = if let Some((min, _)) = heap.peek_min() {
            Timer::after(*min - Instant::now())
        } else {
            Timer::after(Duration::from_secs(86400))
        };

        select! {
            insertion = recv_insertion.next().fuse() => {
                if let Some(insertion) = insertion {
                    let inserted_deadline = Instant::now() + Duration::from_secs(60);
                    heap.push((inserted_deadline, ByAddress(Box::new(insertion))));
                }
            }
            send_response = recv_request.next().fuse() => {
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
    use std::io::prelude::*;
    use std::net::TcpListener;
    use std::thread;
    #[test]
    fn simple() {
        let _ = env_logger::try_init();
        smol::run(async {
            // spawn a stupid echo server that only listens once
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            Task::blocking(async move {
                let (mut cconn, _) = listener.accept().unwrap();
                std::io::copy(&mut cconn.try_clone().unwrap(), &mut cconn);
            })
            .detach();
            println!("done here");
            // connect 5 times; echo must work all the times
            let pool = ConnPool::default();
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
