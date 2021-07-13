use std::{
    collections::BTreeMap,
    net::SocketAddr,
    time::{Duration, Instant},
};

use anyhow::Context;
use futures_util::{StreamExt, TryStreamExt};
use novasmt::CompressedProof;
use themelio_stf::{AbbrBlock, Block, ConsensusProof, NetID, SealedState, Transaction};

use crate::storage::SharedStorage;
use melnet::MelnetError;
use smol::net::TcpListener;
use smol_timeout::TimeoutExt;
use themelio_nodeprot::{NodeClient, NodeResponder, NodeServer, StateSummary, Substate};
use tmelcrypt::HashVal;

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
        listen_addr: SocketAddr,
        advertise_addr: Option<SocketAddr>,
        bootstrap: Vec<SocketAddr>,
        storage: SharedStorage,
    ) -> Self {
        let network = melnet::NetState::new_with_name(netname(netid));
        for addr in bootstrap {
            network.add_route(addr);
        }
        if let Some(advertise_addr) = advertise_addr {
            network.add_route(advertise_addr);
        }
        let responder = AuditorResponder::new(netid, storage.clone());
        network.listen("node", NodeResponder::new(responder));
        let _network_task = smolscale::spawn({
            let network = network.clone();
            async move {
                let listener = TcpListener::bind(listen_addr).await.unwrap();
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

#[tracing::instrument(skip(network, storage))]
async fn blksync_loop(netid: NetID, network: melnet::NetState, storage: SharedStorage) {
    let tag = || {
        format!(
            "blksync@{:?}",
            storage.read().highest_state().header().height
        )
    };
    const SLOW_TIME: Duration = Duration::from_millis(5000);
    const FAST_TIME: Duration = Duration::from_millis(10);
    let mut random_peer = network.routes().first().cloned();
    loop {
        if let Some(peer) = random_peer {
            log::trace!("{}: picked random peer {} for blksync", tag(), peer);
            let client = NodeClient::new(netid, peer);

            let res = attempt_blksync(&client, &storage).await;
            match res {
                Err(e) => {
                    log::warn!("{}: failed to blksync with {}: {:?}", tag(), peer, e);
                    random_peer = network.routes().first().cloned();
                    smol::Timer::after(FAST_TIME).await;
                }
                Ok(blklen) => {
                    if blklen > 0 {
                        log::debug!("synced to height {}", storage.read().highest_height());
                        smol::Timer::after(FAST_TIME).await;
                    } else {
                        smol::Timer::after(SLOW_TIME).await;
                        random_peer = network.routes().first().cloned()
                    }
                }
            }
        } else {
            smol::Timer::after(SLOW_TIME).await;
            random_peer = network.routes().first().cloned()
        }
    }
}

/// Attempts a sync using the given given node client.
async fn attempt_blksync(client: &NodeClient, storage: &SharedStorage) -> anyhow::Result<usize> {
    let their_highest = client
        .get_summary()
        .await
        .context("cannot get their highest block")?
        .height;
    let my_highest = storage.read().highest_height();
    if their_highest <= my_highest {
        return Ok(0);
    }
    let height_stream = futures_util::stream::iter((my_highest..=their_highest).skip(1));
    let lookup_tx = |tx| storage.read().mempool().lookup(tx);
    let mut result_stream = height_stream
        .map(Ok::<_, anyhow::Error>)
        .try_filter_map(|height| async move {
            Ok(Some(async move {
                Ok(client.get_full_block(height, &lookup_tx).await?)
            }))
        })
        .try_buffered(64)
        .boxed();
    let mut toret = 0;
    while let Some(res) = result_stream.try_next().await? {
        let (block, proof): (Block, ConsensusProof) = res;
        log::debug!("fully resolved block {} from network", block.header.height);
        storage
            .write()
            .apply_block(block, proof)
            .context("could not apply a resolved block")?;
        toret += 1;
    }
    Ok(toret)
}

struct AuditorResponder {
    network: NetID,
    storage: SharedStorage,
}

impl NodeServer for AuditorResponder {
    fn send_tx(&self, state: melnet::NetState, tx: Transaction) -> melnet::Result<()> {
        let start = Instant::now();
        let mut storage = self.storage.write();
        let post_lock = Instant::now();
        storage.mempool_mut().apply_transaction(&tx).map_err(|e| {
            // log::warn!("cannot apply tx: {:?}", e);
            MelnetError::Custom(e.to_string())
        })?;
        log::debug!(
            "txhash {}.. inserted ({:?}, {:?} locking, {:?} applying)",
            &tx.hash_nosigs().to_string()[..10],
            start.elapsed(),
            post_lock - start,
            post_lock.elapsed()
        );
        // log::debug!("about to broadcast txhash {:?}", tx.hash_nosigs());
        for neigh in state.routes().iter().take(4).cloned() {
            let tx = tx.clone();
            let network = self.network;
            // log::debug!("bcast {:?} => {:?}", tx.hash_nosigs(), neigh);
            smolscale::spawn(async move {
                NodeClient::new(network, neigh)
                    .send_tx(tx)
                    .timeout(Duration::from_secs(10))
                    .await
            })
            .detach();
        }
        Ok(())
    }

    fn get_abbr_block(&self, height: u64) -> melnet::Result<(AbbrBlock, ConsensusProof)> {
        let storage = self.storage.read();
        let state = storage
            .get_state(height)
            .ok_or_else(|| MelnetError::Custom(format!("block {} not confirmed yet", height)))?;
        let proof = storage
            .get_consensus(height)
            .ok_or_else(|| MelnetError::Custom(format!("block {} not confirmed yet", height)))?;
        Ok((state.to_block().abbreviate(), proof))
    }

    fn get_summary(&self) -> melnet::Result<StateSummary> {
        let storage = self.storage.read();
        let highest = storage.highest_state();
        let proof = storage
            .get_consensus(highest.header().height)
            .unwrap_or_default();
        Ok(StateSummary {
            netid: self.network,
            height: storage.highest_height(),
            header: highest.header(),
            proof,
        })
    }

    fn get_state(&self, height: u64) -> melnet::Result<SealedState> {
        self.storage
            .read()
            .get_state(height)
            .ok_or_else(|| melnet::MelnetError::Custom("no such height".into()))
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
        let (v, proof) = tree.get_with_proof(key.0);
        if !proof.verify(tree.root_hash(), key.0, &v) {
            panic!(
                "get_smt_branch({}, {:?}, {:?}) => {} failed",
                height,
                elem,
                key,
                hex::encode(&v)
            )
        }
        Ok((v.to_vec(), proof.compress()))
    }

    fn get_stakers_raw(&self, height: u64) -> melnet::Result<BTreeMap<HashVal, Vec<u8>>> {
        let state =
            self.storage.read().get_state(height).ok_or_else(|| {
                MelnetError::Custom(format!("block {} not confirmed yet", height))
            })?;
        let mut accum = BTreeMap::new();
        for (k, v) in state.inner_ref().stakes.mapping.iter() {
            accum.insert(HashVal(k), v.to_vec());
        }
        Ok(accum)
    }
}

impl AuditorResponder {
    fn new(network: NetID, storage: SharedStorage) -> Self {
        Self { network, storage }
    }
}
