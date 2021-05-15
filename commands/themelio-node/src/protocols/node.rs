use std::{
    collections::BTreeMap,
    net::SocketAddr,
    time::{Duration, Instant},
};

use autosmt::CompressedProof;
use blkstructs::{ConsensusProof, NetID, Transaction};

use melnet::MelnetError;
use nodeprot::{AbbreviatedBlock, NodeClient, NodeResponder, NodeServer, StateSummary, Substate};
use smol::net::TcpListener;
use tmelcrypt::HashVal;

use crate::services::storage::SharedStorage;

use super::blksync;

/// This encapsulates the node peer-to-peer for both auditors and stakers..
pub struct NodeProtocol {
    _network_task: smol::Task<()>,
    _blksync_task: smol::Task<()>,
}

fn netname(network: NetID) -> &'static str {
    match network {
        NetID::Mainnet => "mainnet-node",
        NetID::Testnet => "testnet-node",
    }
}

impl NodeProtocol {
    /// Creates a new AuditorProtocol listening on the given address with the given AuditorState.
    pub fn new(
        netid: NetID,
        addr: SocketAddr,
        bootstrap: Vec<SocketAddr>,
        storage: SharedStorage,
    ) -> Self {
        let network = melnet::NetState::new_with_name(netname(netid));
        for addr in bootstrap {
            network.add_route(addr);
        }
        network.add_route(addr);
        let responder = AuditorResponder::new(netid, storage.clone());
        network.listen("node", NodeResponder::new(responder));
        let _network_task = smolscale::spawn({
            let network = network.clone();
            async move {
                let listener = TcpListener::bind(addr).await.unwrap();
                network.run_server(listener).await;
            }
        });
        let _blksync_task = smolscale::spawn(blksync_loop(netid, network, storage));
        Self {
            _network_task,
            _blksync_task,
        }
    }
}

#[tracing::instrument(skip(network, state))]
async fn blksync_loop(netid: NetID, network: melnet::NetState, state: SharedStorage) {
    let tag = || format!("blksync@{:?}", state.read().highest_state().header().height);
    const SLOW_TIME: Duration = Duration::from_millis(5000);
    const FAST_TIME: Duration = Duration::from_millis(10);
    loop {
        let random_peer = network.routes().first().cloned();
        if let Some(peer) = random_peer {
            log::trace!("{}: picked random peer {} for blksync", tag(), peer);
            let last_state = state.read().highest_state();
            let start = Instant::now();
            let res = blksync::sync_state(netid, peer, last_state.inner_ref().height + 1, |tx| {
                state.read().mempool().lookup(tx)
            })
            .await;
            match res {
                Err(e) => {
                    log::trace!("{}: failed to blksync with {}: {:?}", tag(), peer, e);
                    smol::Timer::after(FAST_TIME).await;
                }
                Ok(blocks) => {
                    log::debug!(
                        "got {} blocks from {} in {:?}",
                        blocks.len(),
                        peer,
                        start.elapsed()
                    );
                    let blklen = blocks.len();
                    for (blk, cproof) in blocks {
                        let res = state.write().apply_block(blk.clone(), cproof);
                        if res.is_err() {
                            log::warn!(
                                "{}: failed to apply block {} from other node",
                                tag(),
                                blk.header.height
                            );
                            break;
                        }
                    }
                    if blklen > 0 {
                        smol::Timer::after(FAST_TIME).await;
                    } else {
                        smol::Timer::after(SLOW_TIME).await;
                    }
                }
            }
        } else {
            smol::Timer::after(SLOW_TIME).await;
        }
    }
}

struct AuditorResponder {
    network: NetID,
    storage: SharedStorage,
}

impl NodeServer for AuditorResponder {
    fn send_tx(&self, state: melnet::NetState, tx: Transaction) -> melnet::Result<()> {
        self.storage
            .write()
            .mempool_mut()
            .apply_transaction(&tx)
            .map_err(|e| {
                log::warn!("cannot apply tx: {:?}", e);
                MelnetError::Custom(e.to_string())
            })?;
        log::debug!(
            "txhash {:?} successfully inserted, gonna propagate now",
            tx.hash_nosigs()
        );
        // log::debug!("about to broadcast txhash {:?}", tx.hash_nosigs());
        for neigh in state.routes().iter().take(4).cloned() {
            let tx = tx.clone();
            let network = self.network;
            // log::debug!("bcast {:?} => {:?}", tx.hash_nosigs(), neigh);
            smolscale::spawn(async move { NodeClient::new(network, neigh).send_tx(tx).await })
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
        let start = Instant::now();
        let storage = self.storage.read();
        let highest = storage.highest_state();
        let proof = storage
            .get_consensus(highest.header().height)
            .unwrap_or_default();
        dbg!(start.elapsed());
        Ok(StateSummary {
            netid: self.network,
            height: self.storage.read().highest_height(),
            header: highest.header(),
            proof,
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
        if !proof.verify(tree.root_hash(), key, &v) {
            panic!(
                "get_smt_branch({}, {:?}, {:?}) => {} failed",
                height,
                elem,
                key,
                hex::encode(&v)
            )
        }
        Ok((v, proof.compress()))
    }

    fn get_stakers_raw(&self, height: u64) -> melnet::Result<BTreeMap<HashVal, Vec<u8>>> {
        let state =
            self.storage.read().get_state(height).ok_or_else(|| {
                MelnetError::Custom(format!("block {} not confirmed yet", height))
            })?;
        let mut accum = BTreeMap::new();
        for (k, v) in state.inner_ref().stakes.mapping.iter() {
            accum.insert(k, v);
        }
        Ok(accum)
    }
}

impl AuditorResponder {
    fn new(network: NetID, storage: SharedStorage) -> Self {
        Self { network, storage }
    }
}
