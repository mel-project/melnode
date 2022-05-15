use crate::{blkidx::BlockIndexer, storage::NodeStorage};

use std::{
    collections::BTreeMap,
    net::SocketAddr,
    time::{Duration, Instant},
};

use anyhow::Context;
use futures_util::{StreamExt, TryStreamExt};
use lru::LruCache;
use novasmt::{CompressedProof, Database, InMemoryCas, Tree};
use parking_lot::Mutex;
use themelio_stf::SmtMapping;
use themelio_structs::{AbbrBlock, Block, BlockHeight, ConsensusProof, NetID, Transaction, TxHash};

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
        _ => Box::leak(Box::new(format!("{:?}", network))),
    }
}

impl NodeProtocol {
    /// Creates a new AuditorProtocol listening on the given address with the given AuditorState.
    pub fn new(
        netid: NetID,
        listen_addr: SocketAddr,
        advertise_addr: Option<SocketAddr>,
        bootstrap: Vec<SocketAddr>,
        storage: NodeStorage,
        index: bool,
    ) -> Self {
        let network = melnet::NetState::new_with_name(netname(netid));
        for addr in bootstrap {
            network.add_route(addr);
        }
        if let Some(advertise_addr) = advertise_addr {
            network.add_route(advertise_addr);
        }
        let responder = AuditorResponder::new(netid, storage.clone(), index);
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

async fn blksync_loop(netid: NetID, network: melnet::NetState, storage: NodeStorage) {
    let tag = || format!("blksync@{:?}", storage.highest_state().header().height);
    loop {
        let gap_time: Duration = Duration::from_secs_f64(fastrand::f64() * 1.0);
        let routes = network.routes();
        let random_peer = routes.first().cloned();
        if let Some(peer) = random_peer {
            log::trace!("picking peer {} out of {} peers", peer, routes.len());
            let client = NodeClient::new(netid, peer);

            let res = attempt_blksync(peer, &client, &storage).await;
            match res {
                Err(e) => {
                    log::warn!("{}: failed to blksync with {}: {:?}", tag(), peer, e);
                }
                Ok(blklen) => {
                    if blklen > 0 {
                        log::debug!("synced to height {}", storage.highest_height());
                    }
                }
            }
        }
        smol::Timer::after(gap_time).await;
    }
}

/// Attempts a sync using the given given node client.
async fn attempt_blksync(
    addr: SocketAddr,
    client: &NodeClient,
    storage: &NodeStorage,
) -> anyhow::Result<usize> {
    let their_highest = client
        .get_summary()
        .timeout(Duration::from_secs(5))
        .await
        .context("timed out getting summary")?
        .context("cannot get their highest block")?
        .height;
    let my_highest = storage.highest_height();
    if their_highest <= my_highest {
        return Ok(0);
    }
    let height_stream = futures_util::stream::iter((my_highest.0..=their_highest.0).skip(1))
        .map(BlockHeight)
        .take(
            std::env::var("THEMELIO_BLKSYNC_BATCH")
                .ok()
                .and_then(|d| d.parse().ok())
                .unwrap_or(1000),
        );
    let lookup_tx = |tx| storage.mempool().lookup_recent_tx(tx);
    let mut result_stream = height_stream
        .map(Ok::<_, anyhow::Error>)
        .try_filter_map(|height| async move {
            Ok(Some(async move {
                let start = Instant::now();
                let result = client
                    .get_full_block(height, &lookup_tx)
                    .timeout(Duration::from_secs(15))
                    .await
                    .context("timeout")??;
                if result.0.header.height != height {
                    anyhow::bail!("WANTED BLK {}, got {}", height, result.0.header.height);
                }
                log::trace!(
                    "fully resolved block {} from peer {} in {:.2}ms",
                    result.0.header.height,
                    addr,
                    start.elapsed().as_secs_f64() * 1000.0
                );
                Ok(result)
            }))
        })
        .try_buffered(16)
        .boxed();
    let mut toret = 0;
    while let Some(res) = result_stream.try_next().await? {
        let (block, proof): (Block, ConsensusProof) = res;

        storage
            .apply_block(block, proof)
            .await
            .context("could not apply a resolved block")?;
        toret += 1;
    }
    Ok(toret)
}

struct AuditorResponder {
    network: NetID,
    storage: NodeStorage,
    indexer: Option<BlockIndexer>,
    recent: Mutex<LruCache<TxHash, Instant>>,

    coin_smts: Mutex<LruCache<BlockHeight, Tree<InMemoryCas>>>,
}

impl NodeServer for AuditorResponder {
    fn send_tx(&self, state: melnet::NetState, tx: Transaction) -> anyhow::Result<()> {
        if let Some(val) = self.recent.lock().peek(&tx.hash_nosigs()) {
            if val.elapsed().as_secs_f64() < 10.0 {
                anyhow::bail!("rejecting recently seen")
            }
        }
        self.recent.lock().put(tx.hash_nosigs(), Instant::now());
        log::trace!("handling send_tx");
        let start = Instant::now();

        // let start_smt_get = STAT_SMT_GET_SECS.value();
        // let start_smt_insert = STAT_SMT_INSERT_SECS.value();
        // let start_melvm = STAT_MELVM_RUNTIME_SECS.value();
        // let start_melpow = STAT_MELPOW_SECS.value();

        self.storage
            .try_mempool_mut()
            .context("mempool contention")?
            .apply_transaction(&tx)
            .map_err(|e| {
                log::warn!("cannot apply tx: {:?}", e);
                MelnetError::Custom(e.to_string())
            })?;

        log::debug!(
            "txhash {}.. inserted ({:?} applying)",
            &tx.hash_nosigs().to_string()[..10],
            start.elapsed(),
        );

        // log::debug!("about to broadcast txhash {:?}", tx.hash_nosigs());
        for neigh in state.routes().iter().take(16).cloned() {
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

    fn get_abbr_block(&self, height: BlockHeight) -> anyhow::Result<(AbbrBlock, ConsensusProof)> {
        log::trace!("handling get_abbr_block({})", height);
        let state = self
            .storage
            .get_state(height)
            .context(format!("block {} not confirmed yet", height))?;
        let proof = self
            .storage
            .get_consensus(height)
            .context(format!("block {} not confirmed yet", height))?;
        Ok((state.to_block().abbreviate(), proof))
    }

    fn get_summary(&self) -> anyhow::Result<StateSummary> {
        log::trace!("handling get_summary()");
        let highest = self.storage.highest_state();
        let proof = self
            .storage
            .get_consensus(highest.header().height)
            .unwrap_or_default();
        Ok(StateSummary {
            netid: self.network,
            height: highest.inner_ref().height,
            header: highest.header(),
            proof,
        })
    }

    fn get_block(&self, height: BlockHeight) -> anyhow::Result<Block> {
        log::trace!("handling get_state({})", height);
        Ok(self
            .storage
            .get_state(height)
            .context("no such height")?
            .to_block())
    }

    fn get_smt_branch(
        &self,
        height: BlockHeight,
        elem: Substate,
        key: HashVal,
    ) -> anyhow::Result<(Vec<u8>, CompressedProof)> {
        log::trace!("handling get_smt_branch({}, {:?})", height, elem);
        let state = self
            .storage
            .get_state(height)
            .context(format!("block {} not confirmed yet", height))?;
        let ctree = self.get_coin_tree(height)?;
        let (v, proof) = match elem {
            Substate::Coins => state.inner_ref().coins.inner().get_with_proof(key.0),
            Substate::History => state.inner_ref().history.mapping.get_with_proof(key.0),
            Substate::Pools => state.inner_ref().pools.mapping.get_with_proof(key.0),
            Substate::Stakes => state.inner_ref().stakes.mapping.get_with_proof(key.0),
            Substate::Transactions => ctree.get_with_proof(key.0),
        };
        Ok((v.to_vec(), proof.compress()))
    }

    fn get_stakers_raw(&self, height: BlockHeight) -> anyhow::Result<BTreeMap<HashVal, Vec<u8>>> {
        let state = self.storage.get_state(height).context("no such height")?;
        let mut accum = BTreeMap::new();
        for (k, v) in state.inner_ref().stakes.mapping.iter() {
            accum.insert(HashVal(k), v.to_vec());
        }
        Ok(accum)
    }

    fn get_some_coins(
        &self,
        height: BlockHeight,
        covhash: themelio_structs::Address,
    ) -> anyhow::Result<Option<Vec<themelio_structs::CoinID>>> {
        Ok(self
            .indexer
            .as_ref()
            .and_then(|s| s.get(height).map(|idx| idx.lookup(covhash))))
    }
}

impl AuditorResponder {
    fn new(network: NetID, storage: NodeStorage, index: bool) -> Self {
        Self {
            network,
            storage: storage.clone(),
            indexer: if index {
                Some(BlockIndexer::new(storage))
            } else {
                None
            },
            recent: LruCache::new(1000).into(),
            coin_smts: LruCache::new(100).into(),
        }
    }

    fn get_coin_tree(&self, height: BlockHeight) -> anyhow::Result<Tree<InMemoryCas>> {
        let otree = self.coin_smts.lock().get(&height).cloned();
        if let Some(v) = otree {
            Ok(v)
        } else {
            let state = self
                .storage
                .get_state(height)
                .context(format!("block {} not confirmed yet", height))?;
            let mut mm = SmtMapping::new(
                Database::new(InMemoryCas::default())
                    .get_tree(Default::default())
                    .unwrap(),
            );
            for (h, t) in state.inner_ref().transactions.iter() {
                mm.insert(*h, t.clone());
            }
            self.coin_smts.lock().put(height, mm.mapping.clone());
            Ok(mm.mapping)
        }
    }
}
