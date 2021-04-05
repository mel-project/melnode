use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use crate::{
    msg::{Message, SignedMessage},
    Network,
};
use anyhow::Context;
use async_trait::async_trait;
use melnet::NetState;
use smol::channel::{Receiver, Sender};
use smol::prelude::*;
use smol_timeout::TimeoutExt;
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

    send_outgoing: Sender<SignedMessage>,

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
        let (send_outgoing, recv_outgoing) = smol::channel::unbounded();
        let _task = smolscale::spawn(async move {
            let net3 = net2.clone();
            net2.run_server(smol::net::TcpListener::bind(addr).await.unwrap())
                .race(broadcast_loop(net3, recv_outgoing))
                .await;
        });
        Ok(Self {
            network,
            stuff_incoming,
            send_outgoing,
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
            .timeout(Duration::from_secs(10))
            .await
            .ok_or_else(|| anyhow::anyhow!("melnet timeout"))?
            .context(format!("error connecting to {}", random_route))?)
    }
}

#[async_trait]
impl Network for SymphGossip {
    async fn broadcast(&self, msg: SignedMessage) {
        let _ = self.send_outgoing.send(msg).await;
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

// Broadcast loop.
async fn broadcast_loop(network: NetState, recv_outgoing: Receiver<SignedMessage>) {
    loop {
        let msg = recv_outgoing.recv().await;
        if let Ok(msg) = msg {
            let neighs = network.routes();
            neighs.into_iter().take(16).for_each(|neigh| {
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
                .detach();
            });
        } else {
            return;
        }
    }
}
