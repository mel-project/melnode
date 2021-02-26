use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use crate::{
    msg::{ConfirmResp, SignedMessage},
    Network,
};
use async_trait::async_trait;
use smol::channel::Receiver;
use tmelcrypt::Ed25519PK;

const NETNAME: &str = "testnet-staker";

/// A helper structure for the gossip network.
#[derive(Clone)]
pub struct SymphGossip {
    network: melnet::NetState,
    incoming: Receiver<SignedMessage>,
    // a mapping of seqnos to senders
    sender_to_seq: HashMap<Ed25519PK, u64>,
    _task: Arc<smol::Task<()>>,
}

impl SymphGossip {
    pub fn new(addr: SocketAddr, bootstrap: Vec<SocketAddr>) -> anyhow::Result<Self> {
        let network = melnet::NetState::new_with_name(NETNAME);
        for addr in bootstrap {
            network.add_route(addr);
        }
        network.add_route(addr);
        let (send_incoming, incoming) = smol::channel::unbounded();
        network.register_verb(
            "gossip",
            melnet::anon_responder(
                move |req: melnet::Request<SignedMessage, Option<ConfirmResp>>| {
                    let _ = send_incoming.try_send(req.body.clone());
                    eprintln!("RESPOND");
                    req.respond(Ok(None))
                },
            ),
        );
        let net2 = network.clone();
        let _task = smolscale::spawn(async move {
            net2.run_server(smol::net::TcpListener::bind(addr).await.unwrap())
                .await;
        });
        Ok(Self {
            network,
            incoming,
            sender_to_seq: HashMap::new(),
            _task: Arc::new(_task),
        })
    }
}

#[async_trait]
impl Network for SymphGossip {
    async fn broadcast(&self, msg: SignedMessage) {
        let neighs = self.network.routes();
        let bcast_tasks: Vec<_> = neighs
            .into_iter()
            .take(16)
            .map(|neigh| {
                let msg = msg.clone();
                smolscale::spawn(async move {
                    if let Err(err) = melnet::g_client()
                        .request::<_, Option<ConfirmResp>>(neigh, NETNAME, "gossip", msg)
                        .await
                    {
                        log::warn!("error broadcasting: {:?}", err);
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
            log::debug!("got gossip msg {:?}", msg);
            let last_seq = self
                .sender_to_seq
                .get(&msg.sender)
                .cloned()
                .unwrap_or_default();
            if msg.sequence <= last_seq {
                continue;
            }
            self.sender_to_seq.insert(msg.sender, msg.sequence);
            self.broadcast(msg.clone()).await;
            return msg;
        }
    }
}
