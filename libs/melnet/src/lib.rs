//! Melnet serves as Themelio's peer-to-peer network layer, based on a randomized topology and gossip. Peers are divided into servers, which have a publicly reachable address, and clients, which do not. It's based on a simple stdcode request-response protocol, where the only way to "push" a message is to send a request to a server. There is no multiplexing --- the whole thing works like HTTP/1.1. TCP connections are pretty cheap these days.
//!
//! This also means that clients never receive notifications, and must poll servers.
//!
//! The general way to use `melnet` is as follows:
//!
//! 1. Create a `NetState`. This holds the routing table, RPC verb handlers, and other "global" data.
//! 2. If running as a server, register RPC verbs with `NetState::register_verb` and run `NetState::run_server` in the background.
//! 3. Use a `Client`, like the global one returned by `g_client()`, to make RPC calls to other servers. Servers are simply identified by a `std::net::SocketAddr`.

mod connpool;
mod responder;
pub use connpool::g_client;
mod routingtable;
use derivative::*;
use log::{debug, trace};
pub use responder::*;
use routingtable::*;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use std::{collections::HashMap, net::SocketAddr};
mod reqs;
use async_net::{TcpListener, TcpStream};
mod common;
pub use common::*;
use parking_lot::{Mutex, RwLock};
use rand::prelude::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use reqs::*;
use smol::Timer;
use smol_timeout::TimeoutExt;
use std::time::Duration;

#[derive(Derivative, Clone, Default)]
#[derivative(Debug)]
/// A clonable structure representing a melnet state. All copies share the same routing table.
pub struct NetState {
    network_name: String,
    routes: Arc<RwLock<RoutingTable>>,
    #[derivative(Debug = "ignore")]
    verbs: Arc<Mutex<HashMap<String, BoxedResponder>>>,
}

impl NetState {
    /// Runs the netstate. Usually you would want to call this in a separate task. This doesn't consume the netstate because the netstate struct can still be used to get out routes, register new verbs, etc even when it's concurrently run as a server.
    pub async fn run_server(&self, listener: TcpListener) {
        let mut this = self.clone();
        this.setup_routing();
        // Spam neighbors with random routes
        // INTENTIONALLY not detach so that it cancels automatically
        let _spammer = {
            let state = self.clone();
            smolscale::spawn(async move {
                let mut rng = rand::rngs::OsRng {};
                loop {
                    let tmr = Timer::after(Duration::from_secs_f32(0.2));
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
                            .await;
                        match to_wait {
                            Ok(output) => {
                                trace!(
                                    "addrspam sent {:?} to {:?}, output {:?}",
                                    rand_route,
                                    rand_neigh,
                                    output
                                );
                                tmr.await;
                            }
                            Err(_) => {
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
            let (conn, _) = listener.accept().await.unwrap();
            let self_copy = self.clone();
            smolscale::spawn(async move {
                let _ = self_copy.server_handle(conn).await;
            })
            .detach();
        }
    }

    async fn server_handle(&self, mut conn: TcpStream) -> anyhow::Result<()> {
        conn.set_nodelay(true)?;
        loop {
            let opt = self
                .server_handle_one(&mut conn)
                .timeout(Duration::from_secs(60))
                .await;
            match opt {
                None => {
                    break;
                }
                Some(res) => res?,
            }
        }
        Ok(())
    }

    async fn server_handle_one(&self, conn: &mut TcpStream) -> anyhow::Result<()> {
        // read command
        let cmd: RawRequest = stdcode::deserialize(&read_len_bts(conn).await?)?;
        if cmd.proto_ver != 1 {
            let err = stdcode::serialize(&RawResponse {
                kind: "Err".to_owned(),
                body: stdcode::serialize(&"bad protocol version").unwrap(),
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
        let response_fut = {
            let mut verbs = self.verbs.lock();
            let responder = verbs.get_mut(&cmd.verb);
            if let Some(responder) = responder {
                Some((responder.0)(&cmd.payload, conn.clone()))
            } else {
                None
            }
        };
        let response: Result<Vec<u8>> = if let Some(fut) = response_fut {
            fut.await
        } else {
            Err(MelnetError::VerbNotFound)
        };
        match response {
            Ok(resp) => {
                write_len_bts(
                    conn,
                    &stdcode::serialize(&RawResponse {
                        kind: "Ok".into(),
                        body: resp,
                    })
                    .unwrap(),
                )
                .await?
            }
            Err(MelnetError::Custom(string)) => {
                write_len_bts(
                    conn,
                    &stdcode::serialize(&RawResponse {
                        kind: "Err".into(),
                        body: string.as_bytes().into(),
                    })
                    .unwrap(),
                )
                .await?
            }
            Err(MelnetError::VerbNotFound) => {
                write_len_bts(
                    conn,
                    &stdcode::serialize(&RawResponse {
                        kind: "NoVerb".into(),
                        body: b"".to_vec(),
                    })
                    .unwrap(),
                )
                .await?
            }
            err => anyhow::bail!("bad error created by responder: {:?}", err),
        }
        Ok(())
    }

    /// Registers the handler for new_peer.
    fn setup_routing(&mut self) {
        // ping just responds to a u64 with itself
        self.register_verb(
            "ping",
            anon_responder(|ping: Request<u64, _>| {
                let body = ping.body;
                ping.respond(Ok(body))
            }),
        );
        self.register_verb(
            "new_addr",
            anon_responder(|request: Request<RoutingRequest, _>| {
                let rr = request.body.clone();
                let state = request.state.clone();
                let unreach = || MelnetError::Custom(String::from("invalid"));
                if rr.proto != "tcp" {
                    log::debug!("new_addr saw unrecognizable protocol = {:?}", rr.proto);
                    request.respond(Err(MelnetError::Custom("bad protocol".into())));
                    return;
                }
                // move into a task now
                smolscale::spawn(async move {
                    let resp: u64 = g_client()
                        .request(
                            *smol::net::resolve(&rr.addr).await.ok()?.first()?,
                            &state.network_name.to_owned(),
                            "ping",
                            814u64,
                        )
                        .await
                        .ok()?;
                    if resp != 814 {
                        debug!("new_addr bad ping {:?} {:?}", rr.addr, resp);
                        request.respond(Err(unreach()));
                    } else {
                        state.add_route(*smol::net::resolve(&rr.addr).await.ok()?.first()?);
                        request.respond(Ok("".to_string()));
                    }
                    Some(())
                })
                .detach();
            }),
        );
    }

    /// Registers a verb.
    pub fn register_verb<
        Req: DeserializeOwned + Send + 'static,
        Resp: Serialize + Send + 'static,
        T: Responder<Req, Resp> + Send + 'static,
    >(
        &self,
        verb: &str,
        responder: T,
    ) {
        self.verbs
            .lock()
            .insert(verb.into(), responder_to_closure(self.clone(), responder));
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basic_test() {
        let _ = env_logger::try_init();
        let server_task = async {
            let ns = NetState::new_with_name("test");
            // TODO: Fix regsiter verb (does this require a local system up or can we use a test server?)
            // ns.register_verb("test", |_, input: String| async { Ok(input) });
            ns.register_verb(
                "test",
                anon_responder(|_: responder::Request<String, String>| ()),
            );
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
            // let response: String = client
            //     .request(
            //         "127.0.0.1:12345".parse().unwrap(),
            //         "test",
            //         "test",
            //         "hello world",
            //     )
            //     .await
            //     .unwrap();
            // assert_eq!(response, "hello world".to_string())
        };
        smol::future::block_on(smol::future::race(server_task, client_task));
    }
}
