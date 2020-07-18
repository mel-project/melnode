use crate::common::*;
use anyhow::Result;
use futures::channel::oneshot;
use futures::prelude::*;
use futures::select;
use parking_lot::RwLock;
use smol::*;
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
const AUDITOR_NET: &str = "anet1";

/// A structure representing a running auditor (full node).
pub struct Auditor {
    actor: Actor<AuditorMsg>,
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
        state: AuditorState,
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
    state: AuditorState,
    mut net: melnet::NetState,
) -> Actor<AuditorMsg> {
    let state = Arc::new(RwLock::new(state));
    net.register_verb("newtx", {
        let state = state.clone();
        move |ns, tx| {
            let state = state.clone();
            let ns = ns.clone();
            Box::pin(async move { state.write().handle_newtx(&ns, tx) })
        }
    });
    Actor::spawn(move |mut mail| async move {
        let nuu = net.clone().run_server(listener);
        let process_msgs = async move {
            loop {
                match mail.recv().await {
                    GetNet(s) => s.send(net.clone()).unwrap(),
                    NewTx(tx, s) => {
                        let res = state.write().handle_newtx(&net, tx);
                        s.send(
                            try {
                                res?;
                            },
                        )
                        .unwrap();
                    }
                }
            }
        };
        futures::join!(process_msgs, nuu);
    })
}

/// AuditorState represents the internal state of an Auditor. This is consumed when an Auditor is constructed.
pub struct AuditorState {
    mempool: blkstructs::State,
    recent: lru::LruCache<tmelcrypt::HashVal, bool>,
}

impl AuditorState {
    /// Creates an AuditorState for testing, with an in-memory genesis state that puts 1000 mel at the zero-zero coin, unlockable by the always_true script.
    pub fn new_test() -> Self {
        let db = autosmt::DBManager::load(autosmt::MemDB::default());
        let state = blkstructs::State::test_genesis(
            db,
            blkstructs::MICRO_CONVERTER * 1000,
            blkstructs::melscript::Script::always_true().hash(),
        );
        AuditorState {
            mempool: state,
            recent: lru::LruCache::new(65536),
        }
    }

    /// Handles a new transaction.
    fn handle_newtx(
        &mut self,
        ns: &melnet::NetState,
        tx: blkstructs::Transaction,
    ) -> melnet::Result<bool> {
        let txhash = tx.hash_nosigs();
        if self.recent.get(&txhash).is_some() {
            return Ok(false);
        }
        log::debug!(
            "attempting to apply tx {:?} with inputs {:#?} onto state {:?}",
            txhash,
            tx.inputs,
            self.mempool.coins.root_hash()
        );
        let is_new = self.mempool.apply_tx(&tx);
        match is_new {
            Ok(_) => {
                log::debug!("newtx {:?} is new, forwarding!", tx.hash_nosigs());
                let mut routes = ns.routes();
                routes.truncate(16);
                for n in routes {
                    let tx = tx.clone();
                    //let log_str = format!("forwarding {:?} to {:?}", tx.hash_nosigs(), n);
                    //log::debug!("{}", log_str);
                    Task::spawn(async move {
                        let _ = forward_tx(tx, n).await;
                    })
                    .detach();
                }
                self.recent.put(txhash, true);
                Ok(true)
            }
            Err(err) => {
                log::debug!("newtx {:?} rejected: {}", tx.hash_nosigs(), err);
                Err(melnet::MelnetError::Custom(format!("{}", err)))
            }
        }
    }
}

async fn forward_tx(tx: blkstructs::Transaction, dest: SocketAddr) -> melnet::Result<bool> {
    melnet::gcp().request(dest, AUDITOR_NET, "newtx", tx).await
}
