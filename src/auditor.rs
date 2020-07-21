use crate::common::*;
use crate::storage::Storage;
use anyhow::Result;
use futures::channel::oneshot;
use parking_lot::RwLock;
use smol::*;
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
const AUDITOR_NET: &str = "anet1";

/// A structure representing a running auditor (full node).
pub struct Auditor {
    actor: Arc<Actor<AuditorMsg>>,
}

enum AuditorMsg {
    GetNet(oneshot::Sender<melnet::NetState>),
    NewTx(blkstructs::Transaction, oneshot::Sender<Result<()>>),
}
use AuditorMsg::*;

impl Auditor {
    /// Creates a new Auditor from the given listener.
    pub async fn new(
        listener: Async<TcpListener>,
        state: Arc<RwLock<Storage>>,
        bootstrap: Vec<SocketAddr>,
    ) -> Result<Self> {
        let net = new_melnet(&listener, AUDITOR_NET).await?;
        for addr in bootstrap {
            net.add_route(addr)
        }
        let actor = spawn_auditor_actor(listener, state, net);
        Ok(Auditor { actor })
    }

    /// Obtains the underlying melnet network.
    pub async fn get_netstate(&self) -> melnet::NetState {
        use AuditorMsg::*;
        let (s, r) = oneshot::channel();
        self.actor.send(GetNet(s));
        r.await.unwrap()
    }

    /// Sends a transaction.
    pub async fn send_tx(&self, tx: blkstructs::Transaction) -> Result<()> {
        let (s, r) = oneshot::channel();
        self.actor.send(NewTx(tx, s));
        let res = r.await?;
        Ok(res?)
    }
}

fn spawn_auditor_actor(
    listener: Async<TcpListener>,
    state: Arc<RwLock<Storage>>,
    net: melnet::NetState,
) -> Arc<Actor<AuditorMsg>> {
    // hook up callbacks
    let auditor_actor = {
        let net = net.clone();
        Arc::new(Actor::spawn(move |mut mail| async move {
            let _die_with = Task::spawn(net.clone().run_server(listener));
            loop {
                match mail.recv().await {
                    GetNet(s) => s.send(net.clone()).unwrap(),
                    NewTx(tx, s) => {
                        let res = state.write().insert_tx(tx.clone());
                        if res.is_ok() {
                            // hey, it's a good TX! we should tell our friends too!
                            log::debug!(
                                "good tx {:?}, forwarding to up to 16 peers",
                                tx.hash_nosigs()
                            );
                            for dest in net.routes().into_iter().take(16) {
                                let tx = tx.clone();
                                Task::spawn(async move {
                                    let _ = forward_tx(tx, dest).await;
                                })
                                .detach();
                            }
                        }
                        s.send(res).unwrap();
                    }
                }
            }
        }))
    };

    {
        let auditor_actor = auditor_actor.clone();
        net.register_verb("newtx", move |_, tx: blkstructs::Transaction| {
            let auditor_actor = auditor_actor.clone();
            Box::pin(async move {
                let (send, recv) = oneshot::channel();
                auditor_actor.send(NewTx(tx, send));
                Ok(recv.await.is_ok())
            })
        });
    }
    auditor_actor
}

async fn forward_tx(tx: blkstructs::Transaction, dest: SocketAddr) -> melnet::Result<bool> {
    melnet::gcp().request(dest, AUDITOR_NET, "newtx", tx).await
}
