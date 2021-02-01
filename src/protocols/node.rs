use std::{net::SocketAddr, sync::Arc, time::Duration};

use blkstructs::{CoinDataHeight, CoinID, ConsensusProof, Header, Transaction};
use melnet::MelnetError;
use smol::channel::{Receiver, Sender};
use tmelcrypt::HashVal;

use crate::services::storage::SharedStorage;

use super::blksync::{self, AbbreviatedBlock};

/// This encapsulates the node peer-to-peer for both auditors and stakers..
pub struct NodeProtocol {
    network: melnet::NetState,
    responder: Arc<AuditorResponder>,
    _network_task: smol::Task<()>,
    _blksync_task: smol::Task<()>,
}

pub const NODE_NETNAME: &str = "testnet-auditor";

impl NodeProtocol {
    /// Creates a new AuditorProtocol listening on the given address with the given AuditorState.
    pub fn new(
        addr: SocketAddr,
        bootstrap: Vec<SocketAddr>,
        state: SharedStorage,
    ) -> anyhow::Result<Self> {
        let network = melnet::NetState::new_with_name(NODE_NETNAME);
        for addr in bootstrap {
            network.add_route(addr);
        }
        network.add_route(addr);
        let responder = Arc::new(AuditorResponder::new(network.clone(), state.clone()));

        let rr = responder.clone();
        network.register_verb(
            "send_tx",
            melnet::anon_responder(move |req: melnet::Request<Transaction, _>| {
                let txn = req.body.clone();
                req.respond(rr.resp_send_tx(txn))
            }),
        );
        let rr = responder.clone();
        network.register_verb(
            "get_state",
            melnet::anon_responder(move |req: melnet::Request<u64, _>| {
                let body = req.body;
                req.respond(rr.resp_get_state(body))
            }),
        );
        let rr = responder.clone();
        network.register_verb(
            "get_coin_at",
            melnet::anon_responder(move |req: melnet::Request<(u64, CoinID), _>| {
                let body = req.body;
                req.respond(rr.resp_get_coin_at(body.0, body.1))
            }),
        );
        let rr = responder.clone();
        network.register_verb(
            "get_history_at",
            melnet::anon_responder(move |req: melnet::Request<(u64, u64), _>| {
                let body = req.body;
                req.respond(rr.resp_get_history_at(body.0, body.1))
            }),
        );
        let rr = responder.clone();
        network.register_verb(
            "get_tx_at",
            melnet::anon_responder(move |req: melnet::Request<(u64, HashVal), _>| {
                let body = req.body;
                req.respond(rr.resp_get_tx_at(body.0, body.1))
            }),
        );
        let rr = responder.clone();
        network.register_verb(
            "get_last_state",
            melnet::anon_responder(move |req: melnet::Request<(), _>| {
                let _body = req.body;
                req.respond(rr.resp_get_last_state())
            }),
        );
        let rr = responder.clone();
        network.register_verb(
            "get_txx",
            melnet::anon_responder(move |req: melnet::Request<Vec<HashVal>, _>| {
                let resp = rr.resp_get_txx(req.body.clone());
                req.respond(resp)
            }),
        );
        let net2 = network.clone();
        let _network_task = smolscale::spawn(async move {
            net2.run_server(smol::net::TcpListener::bind(addr).await.unwrap())
                .await
        });
        let _blksync_task = smolscale::spawn(blksync_loop(network.clone(), state));
        Ok(Self {
            network,
            responder,
            _blksync_task,
            _network_task,
        })
    }

    /// Broadcasts a transaction into the network.
    pub fn broadcast(&self, txn: Transaction) -> anyhow::Result<()> {
        Ok(self.responder.resp_send_tx(txn)?)
    }
}

#[tracing::instrument(skip(network, state))]
async fn blksync_loop(network: melnet::NetState, state: SharedStorage) {
    let tag = || {
        format!(
            "blksync@{:?}",
            state
                .read()
                .last_block()
                .map(|b| b.inner().inner_ref().height)
        )
    };
    loop {
        let random_peer = network.routes().first().cloned();
        if let Some(peer) = random_peer {
            log::trace!("{}: picked random peer {} for blksync", tag(), peer);
            let last_state = state.read().last_block();
            let res = blksync::sync_state(
                peer,
                NODE_NETNAME,
                last_state.as_ref().map(|v| v.inner().inner_ref()),
                |tx| state.read().get_tx(tx),
            )
            .await;
            match res {
                Err(e) => {
                    log::trace!("{}: failed to blksync with {}: {:?}", tag(), peer, e);
                }
                Ok(None) => {
                    log::trace!("{}: {} didn't have the next block", tag(), peer);
                }
                Ok(Some((blk, cproof))) => {
                    let res = state.write().apply_confirmed_block(blk, cproof);
                    if let Err(e) = res {
                        log::trace!("{}: failed to apply block: {:?}", tag(), e);
                    }
                }
            }
        }
        smol::Timer::after(Duration::from_millis(100)).await;
    }
}

struct AuditorResponder {
    storage: SharedStorage,
    send_tx_bcast: Sender<Transaction>,
}

impl AuditorResponder {
    fn new(network: melnet::NetState, storage: SharedStorage) -> Self {
        let (send_tx_bcast, recv_tx_bcast) = smol::channel::unbounded();
        for _ in 0..16 {
            smolscale::spawn(tx_bcast_loop(network.clone(), recv_tx_bcast.clone())).detach();
        }
        Self {
            storage,
            send_tx_bcast,
        }
    }

    fn resp_send_tx(&self, tx: Transaction) -> melnet::Result<()> {
        self.storage
            .write()
            .insert_tx(tx.clone())
            .map_err(|e| MelnetError::Custom(e.to_string()))?;
        log::debug!(
            "txhash {:?} successfully inserted, gonna propagate now",
            tx.hash_nosigs()
        );
        self.send_tx_bcast
            .try_send(tx)
            .expect("AuditorResponder background task should never exit");
        Ok(())
    }

    fn resp_get_state(&self, height: u64) -> melnet::Result<(AbbreviatedBlock, ConsensusProof)> {
        let storage = self.storage.read();
        let last_block = storage
            .get_history(height)
            .ok_or_else(|| MelnetError::Custom(format!("block {} not confirmed yet", height)))?;
        // create mapping
        Ok((
            AbbreviatedBlock::from_state(last_block.inner()),
            last_block.cproof().clone(),
        ))
    }

    fn resp_get_last_state(&self) -> melnet::Result<(AbbreviatedBlock, ConsensusProof)> {
        let storage = self.storage.read();
        let last_block = storage
            .last_block()
            .ok_or_else(|| MelnetError::Custom("no last block".into()))?;
        // create mapping
        Ok((
            AbbreviatedBlock::from_state(last_block.inner()),
            last_block.cproof().clone(),
        ))
    }

    fn resp_get_coin_at(
        &self,
        height: u64,
        coin_id: CoinID,
    ) -> melnet::Result<(Option<CoinDataHeight>, autosmt::CompressedProof)> {
        let storage = self.storage.read();
        let old_state = storage
            .get_history(height)
            .ok_or_else(|| MelnetError::Custom("no such block in history".into()))?;
        let (res, proof) = old_state.inner().inner_ref().coins.get(&coin_id);
        Ok((res, proof.compress()))
    }

    fn resp_get_history_at(
        &self,
        height: u64,
        history_height: u64,
    ) -> melnet::Result<(Header, autosmt::CompressedProof)> {
        let storage = self.storage.read();
        let old_state = storage
            .get_history(height)
            .ok_or_else(|| MelnetError::Custom("no such block in history".into()))?;
        let (res, proof) = old_state.inner().inner_ref().history.get(&history_height);
        Ok((
            res.ok_or_else(|| MelnetError::Custom("height is in the future".into()))?,
            proof.compress(),
        ))
    }

    fn resp_get_tx_at(
        &self,
        height: u64,
        txhash: HashVal,
    ) -> melnet::Result<(Option<Transaction>, autosmt::CompressedProof)> {
        let storage = self.storage.read();
        let old_state = storage
            .get_history(height)
            .ok_or_else(|| MelnetError::Custom("no such block in history".into()))?;
        let (res, proof) = old_state.inner().inner_ref().transactions.get(&txhash);
        Ok((res, proof.compress()))
    }

    fn resp_get_txx(&self, txx: Vec<tmelcrypt::HashVal>) -> melnet::Result<Vec<Transaction>> {
        let storage = self.storage.read();
        let mut transactions = Vec::new();
        for hash in txx {
            let tx = storage
                .get_tx(hash)
                .ok_or_else(|| MelnetError::Custom("no transaction with this id found".into()))?;
            transactions.push(tx);
        }
        Ok(transactions)
    }
}

/// CSP process that processes transaction broadcasts sequentially. Many are spawned to increase concurrency.
#[tracing::instrument(skip(network, recv_tx_bcast))]
async fn tx_bcast_loop(
    network: melnet::NetState,
    recv_tx_bcast: Receiver<Transaction>,
) -> Option<()> {
    loop {
        let to_cast = recv_tx_bcast.recv().await.ok()?;
        log::debug!("about to broadcast txhash {:?}", to_cast.hash_nosigs());
        for neigh in network.routes().iter().take(4).cloned() {
            log::debug!("bcast {:?} => {:?}", to_cast.hash_nosigs(), neigh);
            smolscale::spawn(melnet::g_client().request::<_, ()>(
                neigh,
                NODE_NETNAME,
                "send_tx",
                to_cast.clone(),
            ))
            .detach();
        }
    }
}
