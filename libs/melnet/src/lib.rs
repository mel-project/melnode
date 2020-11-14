//! Melnet serves as Themelio's peer-to-peer network layer, based on a randomized topology and gossip. Peers are divided into servers, which have a publicly reachable address, and clients, which do not. It's based on a simple bincode request-response protocol, where the only way to "push" a message is to send a request to a server. There is no multiplexing --- the whole thing works like HTTP/1.1. TCP connections are pretty cheap these days.
//!
//! This also means that clients never receive notifications, and must poll servers.
//!
//! The general way to use `melnet` is as follows:
//!
//! 1. Create a `NetState`. This holds the routing table, RPC verb handlers, and other "global" data.
//! 2. If running as a server, register RPC verbs with `NetState::register_verb` and run `NetState::run_server` in the background.
//! 3. Use a `Client`, like the global one returned by `g_client()`, to make RPC calls to other servers. Servers are simply identified by a `std::net::SocketAddr`.

mod connpool;
pub use connpool::g_client;
mod routingtable;
use derivative::*;
use log::{debug, trace};
use routingtable::*;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use std::{
    collections::HashMap,
    net::{SocketAddr, ToSocketAddrs},
};
mod reqs;
use async_net::{TcpListener, TcpStream};
mod common;
pub use common::*;
use futures::prelude::*;
use futures::select;
use parking_lot::RwLock;
use rand::prelude::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use reqs::*;
use smol::Timer;
use std::time::Duration;

type VerbHandler =
    Arc<dyn Fn(&NetState, &[u8]) -> BoxFuture<Result<Vec<u8>>> + Send + Sync + 'static>;

#[derive(Derivative, Clone, Default)]
#[derivative(Debug)]
/// A clonable structure representing a melnet state. All copies share the same routing table.
pub struct NetState {
    network_name: String,
    routes: Arc<RwLock<RoutingTable>>,
    #[derivative(Debug = "ignore")]
    verbs: Arc<RwLock<HashMap<String, VerbHandler>>>,
}

impl NetState {
    /// Runs the netstate. Usually you would want to call this in a separate task. This doesn't consume the netstate because the netstate struct can still be used to get out routes, register new verbs, etc even when it's concurrently run as a server.
    pub async fn run_server(&self, listener: TcpListener) {
        let mut this = self.clone();
        this.setup_routing();
        // Spam neighbors with random routes
        // INTENTIONALLY not detach so that it cancels automatically
        let spammer = {
            let state = self.clone();
            smolscale::spawn(async move {
                let mut rng = rand::rngs::OsRng {};
                loop {
                    let mut tmr = Timer::after(Duration::from_secs_f32(0.2)).fuse();
                    let routes = state.routes.read().to_vec();
                    if !routes.is_empty() {
                        let (rand_neigh, _) = routes[rng.gen::<usize>() % routes.len()];
                        let (rand_route, _) = routes[rng.gen::<usize>() % routes.len()];
                        let to_wait = g_client()
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
                    smolscale::spawn(async move {
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

    async fn server_handle(&self, mut conn: TcpStream) -> anyhow::Result<()> {
        loop {
            select! {
                x = self.server_handle_one(&mut conn).fuse() => x?,
                _ = Timer::after(Duration::from_secs(60)).fuse() => break
            }
        }
        Ok(())
    }

    async fn server_handle_one(&self, conn: &mut TcpStream) -> anyhow::Result<()> {
        // read command
        let cmd: RawRequest = bincode::deserialize(&read_len_bts(conn).await?)?;
        if cmd.proto_ver != 1 {
            let err = bincode::serialize(&RawResponse {
                kind: "Err".to_owned(),
                body: bincode::serialize(&"bad protocol version").unwrap(),
            })
            .unwrap();
            write_len_bts(conn, &err).await?;
            return Err(anyhow::anyhow!("bad"));
        }
        if cmd.netname != self.network_name {
            return Err(anyhow::anyhow!("bad"));
        }
        trace!("got command {:?} from {:?}", cmd, conn.peer_addr());
        // respond to command
        let responder = RwLock::read(&self.verbs).get(&cmd.verb).cloned();
        match responder {
            None => {
                write_len_bts(
                    conn,
                    &bincode::serialize(&RawResponse {
                        kind: "NoVerb".to_owned(),
                        body: b"".to_vec(),
                    })
                    .unwrap(),
                )
                .await?;
                Ok(())
            }
            Some(responder) => {
                let ss = self.clone();
                let responder = responder.clone();
                let response = smol::unblock(move || responder(&ss, &cmd.payload)).await;
                match response.await {
                    Ok(response) => {
                        write_len_bts(
                            conn,
                            &bincode::serialize(&RawResponse {
                                kind: "Ok".to_owned(),
                                body: response,
                            })
                            .unwrap(),
                        )
                        .await?;
                        Ok(())
                    }
                    Err(MelnetError::Custom(err)) => {
                        write_len_bts(
                            conn,
                            &bincode::serialize(&RawResponse {
                                kind: "Err".to_owned(),
                                body: err.as_bytes().to_owned(),
                            })
                            .unwrap(),
                        )
                        .await?;
                        Ok(())
                    }
                    _ => anyhow::bail!("bad"),
                }
            }
        }
    }

    /// Registers the handler for new_peer.
    fn setup_routing(&mut self) {
        // ping just responds to a u64 with itself
        self.register_verb("ping", |_, ping: u64| async move { Ok(ping) });
        // new_addr
        self.register_verb("new_addr", |state, rr: RoutingRequest| async move {
            trace!("got new_addr {:?}", rr);
            let unreach = || MelnetError::Custom(String::from("invalid"));
            if rr.proto != "tcp" {
                debug!("new_addr unrecognizable proto = {:?}", rr.proto);
                return Err(unreach());
            }
            let resp: u64 = g_client()
                .request(
                    rr.addr
                        .to_socket_addrs()
                        .map_err(|_| unreach())?
                        .next()
                        .ok_or_else(unreach)?,
                    &state.network_name.to_owned(),
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
    }

    /// Registers a verb.
    pub fn register_verb<
        TInput: DeserializeOwned + Send,
        TOutput: Serialize + Send,
        F: Future<Output = Result<TOutput>> + Send + 'static,
    >(
        &self,
        verb: &str,
        cback: impl Fn(NetState, TInput) -> F + 'static + Send + Sync,
    ) {
        debug!("registering verb {}", verb);
        // let cback = erase_cback_types(cback);
        RwLock::write(&self.verbs).insert(verb.to_owned(), erase_cback_types(cback));
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
    fn set_name(&mut self, name: &str) {
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

fn erase_cback_types<
    TInput: DeserializeOwned + Send,
    TOutput: Serialize + Send,
    F: Future<Output = Result<TOutput>> + Send + 'static,
>(
    cback: impl Fn(NetState, TInput) -> F + Send + Sync + 'static,
) -> VerbHandler {
    let cback = Arc::new(cback);
    Arc::new(move |state, input| {
        let input = input.to_vec();
        let state = state.clone();
        let cback = cback.clone();
        let fut = async move {
            let output = cback(
                state,
                bincode::deserialize(&input)
                    .map_err(|e| MelnetError::Custom(format!("rlp error: {:?}", e)))?,
            )
            .await?;
            Ok(bincode::serialize(&output).unwrap())
        };
        fut.boxed()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::info;
    #[test]
    fn basic_test() {
        let _ = env_logger::try_init();
        let server_task = async {
            let ns = NetState::new_with_name("test");
            ns.register_verb("test", |_, input: String| async { Ok(input) });
            ns.run_server(
                smol::net::TcpListener::bind("127.0.0.1:12345")
                    .await
                    .unwrap(),
            )
            .await;
        };
        let client_task = async {
            smol::Timer::after(Duration::from_millis(100)).await;
            let client = g_client();
            let response: String = client
                .request(
                    "127.0.0.1:12345".parse().unwrap(),
                    "test",
                    "test",
                    "hello world",
                )
                .await
                .unwrap();
            assert_eq!(response, "hello world".to_string())
        };
        smol::future::block_on(smol::future::race(server_task, client_task));
    }
}
