use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use crate::{msg::SignedMessage, Network};
use async_trait::async_trait;
use smol::channel::{Receiver, Sender};
use tmelcrypt::Ed25519PK;

const NETNAME: &str = "testnet-staker";

const SYMPH_GOSSIP: &str = "symph-gossip";

const CONFIRM_SOLICIT: &str = "confirm-solicit";

/// A helper structure for the gossip network.
#[derive(Clone)]
pub struct SymphGossip {
    network: melnet::NetState,
    stuff_incoming: Sender<SignedMessage>,
    incoming: Receiver<SignedMessage>,
    // a mapping of seqnos to senders
    sender_to_seq: HashMap<Ed25519PK, u64>,
    _task: Arc<smol::Task<()>>,
}

impl SymphGossip {
    /// Creates a new SymphGossip instance from a vector of bootstrap addresses.
    pub fn new(
        addr: SocketAddr,
        bootstrap: Vec<SocketAddr>,
        get_sig: impl Fn(u64) -> Option<(Ed25519PK, Vec<u8>)> + Send + Sync + 'static,
    ) -> anyhow::Result<Self> {
        let network = melnet::NetState::new_with_name(NETNAME);
        for addr in bootstrap {
            network.add_route(addr);
        }
        network.add_route(addr);
        let (send_incoming, incoming) = smol::channel::unbounded();
        let stuff_incoming = send_incoming.clone();
        network.register_verb(
            SYMPH_GOSSIP,
            melnet::anon_responder(move |req: melnet::Request<SignedMessage, ()>| {
                let _ = send_incoming.try_send(req.body.clone());
                req.respond(Ok(()))
            }),
        );
        network.register_verb(
            CONFIRM_SOLICIT,
            melnet::anon_responder(move |req: melnet::Request<u64, _>| {
                let body = req.body;
                req.respond(Ok(get_sig(body)))
            }),
        );
        let net2 = network.clone();
        let _task = smolscale::spawn(async move {
            net2.run_server(smol::net::TcpListener::bind(addr).await.unwrap())
                .await;
        });
        Ok(Self {
            network,
            stuff_incoming,
            incoming,
            sender_to_seq: HashMap::new(),
            _task: Arc::new(_task),
        })
    }

    /// Solicits *some* confirmation signature for a block.
    pub async fn solicit_confirmation(
        &self,
        height: u64,
    ) -> anyhow::Result<Option<(Ed25519PK, Vec<u8>)>> {
        let random_route = *self.network.routes().first().unwrap();
        Ok(melnet::g_client()
            .request(random_route, NETNAME, CONFIRM_SOLICIT, height)
            .await?)
    }
}

#[async_trait]
impl Network for SymphGossip {
    async fn broadcast(&self, msg: SignedMessage) {
        let _ = self.stuff_incoming.try_send(msg.clone());
        let neighs = self.network.routes();
        let bcast_tasks: Vec<_> = neighs
            .into_iter()
            .take(16)
            .map(|neigh| {
                let msg = msg.clone();
                smolscale::spawn(async move {
                    if let Err(err) = melnet::g_client()
                        .request::<_, ()>(neigh, NETNAME, SYMPH_GOSSIP, msg)
                        .await
                    {
                        log::warn!("error broadcasting to {}: {:?}", neigh, err);
                        return;
                    }
                })
            })
            .collect();
        for task in bcast_tasks {
            task.await
        }
    }

    async fn receive(&mut self) -> SignedMessage {
        loop {
            // receive a message
            let msg = self
                .incoming
                .recv()
                .await
                .expect("melnet task somehow died");
            if msg.body().is_none() {
                continue;
            }
            log::trace!("got gossip msg from {:?}", msg.sender);
            let last_seq = self
                .sender_to_seq
                .get(&msg.sender)
                .cloned()
                .unwrap_or_default();
            if msg.sequence <= last_seq {
                log::trace!("discarding duplicate message");
                continue;
            }
            self.sender_to_seq.insert(msg.sender, msg.sequence);
            let this = self.clone();
            let msg2 = msg.clone();
            smolscale::spawn(async move { this.broadcast(msg2).await }).detach();
            return msg;
        }
    }
}
