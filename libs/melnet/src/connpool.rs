use by_address::ByAddress;
use crossbeam_channel::{after, bounded, select, Receiver, Sender};
use log::trace;
use min_max_heap::MinMaxHeap;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::thread;
use std::time::{Duration, Instant};

/// Implements a thread-safe pool of connections to melnet, or any HTTP/1.1-style keepalive protocol, servers.
#[derive(Debug, Default)]
pub struct ConnPool {
    pool: RwLock<HashMap<SocketAddr, SingleHost>>,
}

impl ConnPool {
    // Connects to a given address, which may return either a new connection or an existing one.
    pub fn connect(&self, addr: impl ToSocketAddrs) -> std::io::Result<TcpStream> {
        let addr = addr.to_socket_addrs()?.next().unwrap();
        let existing = self.pool.read().get(&addr).and_then(|sh| sh.get_conn());
        match existing {
            Some(existing) => {
                trace!("connect({:?}) -> reuse", addr);
                Ok(existing)
            }
            None => {
                // create a new connection
                trace!("connect({:?}) -> fresh", addr);
                TcpStream::connect_timeout(&addr, Duration::from_secs(5))
            }
        }
    }
    // Takes ownership of and returns a given TCP connection back to the pool.
    pub fn recycle(&self, conn: TcpStream) {
        let addr = conn.peer_addr().unwrap();
        trace!("recycle({:?})", addr);
        self.pool
            .write()
            .entry(addr)
            .or_insert_with(SingleHost::default)
            .send_insertion
            .send(conn)
            .unwrap();
    }
}

#[derive(Debug)]
struct SingleHost {
    send_insertion: Sender<TcpStream>,
    send_request: Sender<Sender<Option<TcpStream>>>,
}

impl Default for SingleHost {
    fn default() -> Self {
        let (send_insertion, recv_insertion) = bounded(0);
        let (send_request, recv_request) = bounded(0);
        thread::spawn(move || singlehost_monitor(recv_insertion, recv_request));
        SingleHost {
            send_insertion,
            send_request,
        }
    }
}

impl SingleHost {
    fn get_conn(&self) -> Option<TcpStream> {
        let (send, recv) = bounded(0);
        self.send_request.send(send).unwrap();
        recv.recv().unwrap()
    }
}

fn singlehost_monitor(
    recv_insertion: Receiver<TcpStream>,
    recv_request: Receiver<Sender<Option<TcpStream>>>,
) {
    let mut heap = MinMaxHeap::new();
    loop {
        let deadline = if let Some((min, _)) = heap.peek_min() {
            after(*min - Instant::now())
        } else {
            after(Duration::from_secs(86400))
        };
        select! {
            recv(recv_insertion) -> insertion => {
                if let Ok(insertion) = insertion {
                    let inserted_deadline = Instant::now() + Duration::from_secs(60);
                    heap.push((inserted_deadline, ByAddress(Box::new(insertion))))
                } else {
                    return
                }
            }
            recv(recv_request) -> send_response => {
                if let Ok(send_response) = send_response {
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
            recv(deadline) -> _ => {
                let to_run = heap.pop_min();
                if let Some((_, tcp)) = to_run {
                    drop(tcp)
                }
            }
        }
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
        // spawn a stupid echo server that only listens once
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut cconn, _) = listener.accept().unwrap();
            std::io::copy(&mut cconn.try_clone().unwrap(), &mut cconn);
        });
        // connect 5 times; echo must work all the times
        let pool = ConnPool::default();
        for count in 0..5 {
            let test_str = format!("echo-{}", count);
            let mut conn = pool.connect(addr).unwrap();
            conn.write_all(&test_str.clone().into_bytes()).unwrap();
            let mut buf = [0; 6];
            conn.read_exact(&mut buf).unwrap();
            assert_eq!(buf.to_vec(), test_str.into_bytes());
            pool.recycle(conn)
        }
    }
}
