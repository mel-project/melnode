use std::{net::SocketAddr, sync::Arc, time::Duration};

use autosmt::CompressedProof;
use blkstructs::{CoinDataHeight, CoinID, ConsensusProof, Header, NetID, Transaction};
use fastsync::send_fastsync;
use melnet::MelnetError;
use neosymph::TxLookup;
use nodeprot::{AbbreviatedBlock, NodeClient, NodeResponder, NodeServer, StateSummary, Substate};
use smol::{
    channel::{Receiver, Sender},
    net::TcpListener,
};
use tmelcrypt::HashVal;

use crate::services::storage::SharedStorage;

use super::blksync;

mod fastsync;

/// This encapsulates the node peer-to-peer for both auditors and stakers..
pub struct NodeProtocol {
    _network_task: smol::Task<()>,
    _blksync_task: smol::Task<()>,
}

pub const NODE_NETNAME: &str = "testnet-node";

impl NodeProtocol {
    /// Creates a new AuditorProtocol listening on the given address with the given AuditorState.
    pub fn new(addr: SocketAddr, bootstrap: Vec<SocketAddr>, storage: SharedStorage) -> Self {
        let network = melnet::NetState::new_with_name(NODE_NETNAME);
        for addr in bootstrap {
            network.add_route(addr);
        }
        network.add_route(addr);
        let responder = AuditorResponder::new(storage.clone());
        network.register_verb("node", NodeResponder::new(responder));
        let _network_task = smolscale::spawn({
            let network = network.clone();
            async move {
                let listener = TcpListener::bind(addr).await.unwrap();
                network.run_server(listener).await;
            }
        });
        let _blksync_task = smolscale::spawn(blksync_loop(network, storage));
        Self {
            _network_task,
            _blksync_task,
        }
    }
}

#[tracing::instrument(skip(network, state))]
async fn blksync_loop(network: melnet::NetState, state: SharedStorage) {
    let tag = || format!("blksync@{:?}", state.read().highest_state());
    loop {
        let random_peer = network.routes().first().cloned();
        if let Some(peer) = random_peer {
            log::trace!("{}: picked random peer {} for blksync", tag(), peer);
            let last_state = state.read().highest_state();
            let res = blksync::sync_state(peer, last_state.inner_ref().height + 1, |tx| {
                state.read().mempool().lookup(tx)
            })
            .await;
            match res {
                Err(e) => {
                    log::trace!("{}: failed to blksync with {}: {:?}", tag(), peer, e);
                }
                Ok(blocks) => {
                    for (blk, cproof) in blocks {
                        let res = state.write().apply_block(blk.clone(), cproof);
                        if let Err(e) = res {
                            log::warn!("{:#?}", blk);
                            log::warn!("{}: failed to apply block from other node: {:?}", tag(), e);
                        }
                    }
                }
            }
        }
        smol::Timer::after(Duration::from_millis(100)).await;
    }
}

struct AuditorResponder {
    storage: SharedStorage,
}

impl NodeServer for AuditorResponder {
    fn send_tx(&self, state: melnet::NetState, tx: Transaction) -> melnet::Result<()> {
        self.storage
            .write()
            .mempool_mut()
            .apply_transaction(&tx)
            .map_err(|e| MelnetError::Custom(e.to_string()))?;
        log::debug!(
            "txhash {:?} successfully inserted, gonna propagate now",
            tx.hash_nosigs()
        );
        log::debug!("about to broadcast txhash {:?}", tx.hash_nosigs());
        for neigh in state.routes().iter().take(4).cloned() {
            let tx = tx.clone();
            log::debug!("bcast {:?} => {:?}", tx.hash_nosigs(), neigh);
            smolscale::spawn(
                async move { NodeClient::new(NetID::Testnet, neigh).send_tx(tx).await },
            )
            .detach();
        }
        Ok(())
    }

    fn get_abbr_block(&self, height: u64) -> melnet::Result<(AbbreviatedBlock, ConsensusProof)> {
        let storage = self.storage.read();
        let state = storage
            .get_state(height)
            .ok_or_else(|| MelnetError::Custom(format!("block {} not confirmed yet", height)))?;
        let proof = storage
            .get_consensus(height)
            .ok_or_else(|| MelnetError::Custom(format!("block {} not confirmed yet", height)))?;
        Ok((AbbreviatedBlock::from_state(&state), proof))
    }

    fn get_summary(&self) -> melnet::Result<StateSummary> {
        Ok(StateSummary {
            netid: NetID::Testnet,
            last_height: self.storage.read().highest_height(),
        })
    }

    fn get_smt_branch(
        &self,
        height: u64,
        elem: Substate,
        key: HashVal,
    ) -> melnet::Result<(Vec<u8>, CompressedProof)> {
        let state =
            self.storage.read().get_state(height).ok_or_else(|| {
                MelnetError::Custom(format!("block {} not confirmed yet", height))
            })?;
        let tree = match elem {
            Substate::Coins => &state.inner_ref().coins.mapping,
            Substate::History => &state.inner_ref().history.mapping,
            Substate::Pools => &state.inner_ref().pools.mapping,
            Substate::Stakes => &state.inner_ref().stakes.mapping,
            Substate::Transactions => &state.inner_ref().transactions.mapping,
        };
        let (v, proof) = tree.get(key);
        Ok((v, proof.compress()))
    }
}

impl AuditorResponder {
    fn new(storage: SharedStorage) -> Self {
        Self { storage }
    }
}

/// CSP process that processes transaction broadcasts sequentially. Many are spawned to increase concurrency.
#[tracing::instrument(skip(network, recv_tx_bcast))]
async fn tx_bcast(network: melnet::NetState, recv_tx_bcast: Receiver<Transaction>) -> Option<()> {
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
