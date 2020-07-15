use rlp::{Decodable, Encodable};
mod connpool;
pub use connpool::gcp;
mod routingtable;
use derivative::*;
use log::{debug, trace};
use routingtable::*;
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::Arc;
mod reqs;
use futures::prelude::*;
use futures::select;
use im::HashMap;
use parking_lot::RwLock;
use rand::prelude::*;
use reqs::*;
use smol::*;
use std::time::Duration;
mod common;
pub use common::*;
use rand::seq::SliceRandom;
use rand::thread_rng;

type VerbHandler = Arc<dyn Fn(&NetState, &[u8]) -> BoxFuture<Result<Vec<u8>>> + Send + Sync>;

#[derive(Derivative, Clone, Default)]
#[derivative(Debug)]
/// A clonable structure representing a melnet state. All copies share the same routing table.
pub struct NetState {
    network_name: String,
    routes: Arc<RwLock<RoutingTable>>,
    #[derivative(Debug = "ignore")]
    verbs: HashMap<String, VerbHandler>,
}

impl NetState {
    /// Consumes the netstate and runs it, blocking until the listener no longer gives things. You should clone a copy first in order to use the netstate as a client.
    pub async fn run_server(mut self, listener: Async<TcpListener>) {
        self.setup_routing();
        // Spam neighbors with random routes
        // INTENTIONALLY not detach so that it cancels automatically
        let spammer = {
            let state = self.clone();
            Task::spawn(async move {
                let mut rng = rand::rngs::OsRng {};
                loop {
                    let mut tmr = Timer::after(Duration::from_secs_f32(0.2)).fuse();
                    let routes = state.routes.read().to_vec();
                    if !routes.is_empty() {
                        let (rand_neigh, _) = routes[rng.gen::<usize>() % routes.len()];
                        let (rand_route, _) = routes[rng.gen::<usize>() % routes.len()];
                        let to_wait = gcp()
                            .request::<RoutingRequest, String>(
                                rand_neigh,
                                &state.network_name,
                                "new_addr",
                                RoutingRequest {
                                    proto: String::from("tcp"),
                                    addr: rand_route.to_string(),
                                },
                            )
                            .fuse();
                        select! {
                            output = Box::pin(to_wait) => {
                                trace!("addrspam sent {:?} to {:?}, output {:?}", rand_route, rand_neigh, output);
                                tmr.await;
                            }
                            _ = tmr => {
                                trace!("addrspam timer expired on {:?}, switching...", rand_neigh)
                            }
                        }
                    } else {
                        debug!("addrspam no neighbors, sleeping...");
                        tmr.await;
                    }
                }
            })
        };
        loop {
            let conn = listener.accept().await;
            let self_copy = self.clone();
            match conn {
                Ok((conn, _)) => {
                    //let conn: Async<TcpStream> = conn;
                    Task::spawn(async move {
                        let _ = self_copy.server_handle(conn).await;
                    })
                    .detach();
                }
                Err(err) => {
                    debug!("exiting listener due to {:?}", err);
                    spammer.cancel().await;
                    return;
                }
            }
        }
    }

    async fn server_handle(
        &self,
        mut conn: Async<TcpStream>,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        conn.get_ref()
            .set_write_timeout(Some(std::time::Duration::from_secs(10)))?;
        conn.get_ref()
            .set_read_timeout(Some(std::time::Duration::from_secs(60)))?;
        loop {
            // read command
            let cmd: RawRequest = rlp::decode(&read_len_bts(&mut conn).await?)?;
            if cmd.proto_ver != 1 {
                let err = rlp::encode(&RawResponse {
                    kind: "Err".to_owned(),
                    body: rlp::encode(&"bad protocol version"),
                });
                write_len_bts(&mut conn, &err).await?;
                continue;
            }
            if cmd.netname != self.network_name {
                return Err(Box::new(MelnetError::Custom("bad".to_string())));
            }
            trace!(
                "got command {:?} from {:?}",
                cmd,
                conn.get_ref().peer_addr()
            );
            // respond to command
            let responder = self.verbs.get(&cmd.verb);
            match responder {
                None => {
                    write_len_bts(
                        &mut conn,
                        &rlp::encode(&RawResponse {
                            kind: "NoVerb".to_owned(),
                            body: b"".to_vec(),
                        }),
                    )
                    .await?;
                    continue;
                }
                Some(responder) => {
                    let ss = self.clone();
                    let responder = responder.clone();
                    let response = responder(&ss, &cmd.payload).await;
                    match response {
                        Ok(response) => {
                            write_len_bts(
                                &mut conn,
                                &rlp::encode(&RawResponse {
                                    kind: "Ok".to_owned(),
                                    body: response,
                                }),
                            )
                            .await?;
                        }
                        Err(MelnetError::Custom(err)) => {
                            write_len_bts(
                                &mut conn,
                                &rlp::encode(&RawResponse {
                                    kind: "Err".to_owned(),
                                    body: err.as_bytes().to_owned(),
                                }),
                            )
                            .await?
                        }
                        _ => break,
                    }
                }
            }
        }
        Ok(())
    }

    /// Registers the handler for new_peer.
    fn setup_routing(&mut self) {
        // ping just responds to a u64 with itself
        self.register_verb("ping", |_, ping: u64| Box::pin(async move { Ok(ping) }));
        // new_addr
        self.register_verb("new_addr", |state, rr: RoutingRequest| {
            trace!("got new_addr {:?}", rr);
            let state = state.clone();
            Box::pin(async move {
                let unreach = || MelnetError::Custom(String::from("invalid"));
                if rr.proto != "tcp" {
                    debug!("new_addr unrecognizable proto = {:?}", rr.proto);
                    return Err(unreach());
                }
                let resp: u64 = gcp()
                    .request(
                        rr.addr
                            .to_socket_addrs()
                            .map_err(|_| unreach())?
                            .next()
                            .ok_or_else(unreach)?,
                        &state.network_name,
                        "ping",
                        814 as u64,
                    )
                    .await
                    .map_err(|_| unreach())?;
                if resp != 814 as u64 {
                    debug!("new_addr bad ping {:?} {:?}", rr.addr, resp);
                    return Err(unreach());
                }
                state
                    .routes
                    .write()
                    .add_route(string_to_addr(&rr.addr).ok_or_else(unreach)?);
                trace!("new_addr processed {:?}", rr.addr);
                Ok("")
            })
        })
    }

    /// Registers a verb that takes in an RLP object and returns an RLP object.
    pub fn register_verb<TInput: Decodable + Send, TOutput: Encodable + Send>(
        &mut self,
        verb: &str,
        cback: impl Fn(&NetState, TInput) -> BoxFuture<Result<TOutput>> + 'static + Send + Sync,
    ) {
        debug!("registering verb {}", verb);
        self.verbs.insert(verb.to_owned(), erase_cback_types(cback));
    }

    /// Adds a route to the routing table.
    pub fn add_route(&self, addr: SocketAddr) {
        self.routes.write().add_route(addr)
    }

    /// Obtains a vector of routes. This is guaranteed to be uniformly shuffled, so taking the first N elements is always fair.
    pub fn routes(&self) -> Vec<SocketAddr> {
        let mut rr: Vec<SocketAddr> = self.routes.read().to_vec().iter().map(|v| v.0).collect();
        rr.shuffle(&mut thread_rng());
        rr
    }

    /// Sets the name of the network state.
    pub fn set_name(&mut self, name: &str) {
        self.network_name = name.to_string()
    }

    /// Constructs a netstate with a given name.
    pub fn new_with_name(name: &str) -> Self {
        let mut ns = NetState::default();
        ns.set_name(name);
        ns
    }
}

fn string_to_addr(s: &str) -> Option<SocketAddr> {
    s.to_socket_addrs().ok()?.next()
}

fn erase_cback_types<TInput: Decodable + Send, TOutput: Encodable + Send>(
    cback: impl Fn(&NetState, TInput) -> BoxFuture<Result<TOutput>> + 'static + Send + Sync,
) -> VerbHandler {
    let cback = Arc::new(cback);
    Arc::new(move |state, input| {
        let input = input.to_vec();
        let state = state.clone();
        let cback = cback.clone();
        Box::pin(async move {
            let input: TInput = rlp::decode(&input)
                .map_err(|e| MelnetError::Custom(format!("rlp error: {:?}", e)))?;
            let output: TOutput = cback(&state, input).await?;
            Ok(rlp::encode(&output))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::info;
    #[test]
    fn basic_test() {
        let _ = env_logger::try_init();
        run(async {
            let task = Task::local(async {
                const NUM: usize = 100;
                // start listeners
                let listeners: Vec<Async<TcpListener>> = (0..NUM)
                    .map(|i| {
                        info!("starting listener {}", i);
                        let listener = Async::<TcpListener>::bind("127.0.0.1:0").unwrap();
                        listener
                    })
                    .collect();
                info!("listeners made");
                let first_addr = listeners[0].get_ref().local_addr().unwrap();
                // start tasks
                let tasks: Vec<_> = listeners
                    .into_iter()
                    .map(|listener| {
                        let nstate = NetState::new_with_name("testnet");
                        nstate.add_route(first_addr.clone());
                        nstate.add_route(listener.get_ref().local_addr().unwrap());
                        Task::spawn(nstate.run_server(listener))
                    })
                    .collect();
                for t in tasks {
                    t.await
                }
            })
            .boxed_local();
            select! {
                _ = task.fuse() => panic!("tasks ended?!"),
                _ = Timer::after(Duration::from_secs(5)).fuse() => info!("ending things now")
            }
        });
    }
}
