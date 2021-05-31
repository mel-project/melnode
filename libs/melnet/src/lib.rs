//! Melnet serves as Themelio's peer-to-peer network layer, based on a randomized topology and gossip. Peers are divided into servers, which have a publicly reachable address, and clients, which do not. It's based on a simple stdcode request-response protocol, where the only way to "push" a message is to send a request to a server. There is no multiplexing --- the whole thing works like HTTP/1.1. TCP connections are pretty cheap these days.
//!
//! This also means that clients never receive notifications, and must poll servers.
//!
//! The general way to use `melnet` is as follows:
//!
//! 1. Create a `NetState`. This holds the routing table, RPC verb handlers, and other "global" data.
//! 2. If running as a server, register RPC verbs with `NetState::register_verb` and run `NetState::run_server` in the background.
//! 3. Use a `Client`, like the global one returned by `g_client()`, to make RPC calls to other servers. Servers are simply identified by a `std::net::SocketAddr`.

mod client;
mod endpoint;
mod routingtable;
use derivative::*;
pub use endpoint::*;
use log::{debug, trace};
use routingtable::*;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use std::{collections::HashMap, net::SocketAddr};
mod reqs;
use async_net::{TcpListener, TcpStream};
mod common;
pub use client::request;
pub use common::*;
use parking_lot::{Mutex, RwLock};
use rand::prelude::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use reqs::*;
use smol::{channel::Receiver, Timer};
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
                        let to_wait = crate::request::<RoutingRequest, String>(
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

        // Max number of connections
        const MAX_CONNECTIONS: usize = 256;
        let conn_semaphore = smol::lock::Semaphore::new(MAX_CONNECTIONS);
        let (conn_abort_send, conn_abort_recv) = smol::channel::unbounded::<()>();
        loop {
            let (conn, addr) = listener.accept().await.unwrap();
            let self_copy = self.clone();
            if let Some(_guard) = conn_semaphore.try_acquire() {
                let conn_abort_recv = conn_abort_recv.clone();
                smolscale::spawn(async move {
                    if let Some(Err(e)) = self_copy
                        .server_handle(conn, conn_abort_recv)
                        .timeout(Duration::from_secs(120))
                        .await
                    {
                        log::debug!("{} terminating on error: {:?}", addr, e)
                    }
                })
                .detach();
            } else {
                log::warn!("too many connections, rejecting an accepted connection and aborting an existing one!");
                conn_abort_send.try_send(()).unwrap();
            }
        }
    }

    async fn server_handle(
        &self,
        mut conn: TcpStream,
        conn_abort_recv: Receiver<()>,
    ) -> anyhow::Result<()> {
        conn.set_nodelay(true)?;
        loop {
            self.server_handle_one(&mut conn).await?;
            if conn_abort_recv.try_recv().is_ok() {
                anyhow::bail!("aborting on too-many-connections signal")
            }
        }
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
            let responder = self.verbs.lock().get(&cmd.verb).cloned();
            if let Some(responder) = responder {
                let res = responder.0(&cmd.payload);
                Some(res)
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
        self.listen("ping", |ping: Request<u64, _>| {
            let body = ping.body;
            ping.response.send(Ok(body))
        });
        self.listen("new_addr", |request: Request<RoutingRequest, _>| {
            let rr = request.body.clone();
            let state = request.state.clone();
            let unreach = || MelnetError::Custom(String::from("invalid"));
            if rr.proto != "tcp" {
                log::debug!("new_addr saw unrecognizable protocol = {:?}", rr.proto);
                request
                    .response
                    .send(Err(MelnetError::Custom("bad protocol".into())));
                return;
            }
            // move into a task now
            smolscale::spawn(async move {
                let resp: u64 = crate::request(
                    *smol::net::resolve(&rr.addr).await.ok()?.first()?,
                    &state.network_name.to_owned(),
                    "ping",
                    814u64,
                )
                .await
                .ok()?;
                if resp != 814 {
                    debug!("new_addr bad ping {:?} {:?}", rr.addr, resp);
                    request.response.send(Err(unreach()));
                } else {
                    state.add_route(*smol::net::resolve(&rr.addr).await.ok()?.first()?);
                    request.response.send(Ok("".to_string()));
                }
                Some(())
            })
            .detach();
        });
    }

    /// Registers a verb.
    pub fn listen<
        Req: DeserializeOwned + Send + 'static,
        Resp: Serialize + Send + 'static,
        T: Endpoint<Req, Resp> + Send + 'static,
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
