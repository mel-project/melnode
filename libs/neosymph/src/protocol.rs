use std::{net::SocketAddr, sync::Arc, time::Duration};

use crate::{
    msg::{Message, SignedMessage},
    Network,
};
use anyhow::Context;
use async_trait::async_trait;
use melnet::NetState;
use msgstate::{MsgState, MsgStateDiff, MsgStateStatus};
use parking_lot::Mutex;
use smol::channel::{Receiver, Sender};
use smol::prelude::*;
use smol_timeout::TimeoutExt;
use tmelcrypt::Ed25519PK;
mod msgstate;
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
    messages: Arc<Mutex<MsgState>>,
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
        let messages = Arc::new(Mutex::new(MsgState::default()));
        {
            let messages = messages.clone();
            network.listen(
                SYMPH_GOSSIP,
                move |req: melnet::Request<MsgStateStatus, MsgStateDiff>| {
                    let status = req.body.clone();
                    let diff = messages.lock().oneside_diff(status);
                    req.response.send(Ok(diff))
                },
            );
        }
        network.listen(CONFIRM_SOLICIT, move |req: melnet::Request<u64, _>| {
            let body = req.body;
            req.response.send(Ok(get_sig(body)))
        });
        let net2 = network.clone();
        let msg2 = messages.clone();
        let sinc = send_incoming.clone();
        let _task = smolscale::spawn(async move {
            let net3 = net2.clone();
            net2.run_server(smol::net::TcpListener::bind(addr).await.unwrap())
                .race(broadcast_loop(net3, msg2, sinc))
                .await;
        });
        Ok(Self {
            stuff_incoming: send_incoming,
            network,
            incoming,
            messages,
            _task: Arc::new(_task),
        })
    }

    /// Solicits *some* confirmation signature for a block.
    pub async fn solicit_confirmation(
        &self,
        height: u64,
    ) -> anyhow::Result<Option<(Ed25519PK, Vec<u8>)>> {
        let random_route = *self.network.routes().first().unwrap();
        Ok(
            melnet::request(random_route, NETNAME, CONFIRM_SOLICIT, height)
                .timeout(Duration::from_secs(10))
                .await
                .ok_or_else(|| anyhow::anyhow!("melnet timeout"))?
                .context(format!("error connecting to {}", random_route))?,
        )
    }
}

#[async_trait]
impl Network for SymphGossip {
    async fn broadcast(&self, msg: SignedMessage) {
        let _ = self.stuff_incoming.try_send(msg.clone());
        log::warn!("broadcasted {:?}", msg);
        self.messages.lock().insert(msg);
    }

    async fn receive(&mut self) -> SignedMessage {
        loop {
            // receive a message
            let msg = self
                .incoming
                .recv()
                .await
                .expect("melnet task somehow died");
            log::warn!("received {:?}", msg);
            if msg.body().is_none() {
                log::warn!("*** BAILING!!!! ***");
                continue;
            }
            return msg;
        }
    }
}

// Broadcast loop.
async fn broadcast_loop(
    network: NetState,
    messages: Arc<Mutex<MsgState>>,
    send_incoming: Sender<SignedMessage>,
) {
    loop {
        let status = messages.lock().snapshot();
        // log::trace!("status obtained: {:?}", status);
        let neighbor = network.routes()[0];

        let res = melnet::request(neighbor, NETNAME, SYMPH_GOSSIP, status)
            .timeout(Duration::from_secs(10))
            .await;
        match res {
            Some(Ok(diff)) => {
                // log::trace!("diff obtained: {:?}", diff);
                let mut new_msgs = messages.lock().apply_diff(diff);
                // we sort the new messages by putting all the proposals before all the votes.
                // this prevents 'dangling votes'
                new_msgs.sort_by_key(|v| match &v.body() {
                    Some(Message::Proposal(..)) => 0,
                    _ => 1,
                });
                for msg in new_msgs {
                    let _ = send_incoming.try_send(msg);
                }
            }
            other => log::warn!("broadcast_loop to {} error: {:?}", neighbor, other),
        }
        smol::Timer::after(Duration::from_millis(100)).await;
    }
}
