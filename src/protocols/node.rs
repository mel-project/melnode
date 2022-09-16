use crate::{blkidx::BlockIndexer, storage::Storage};

use std::{
    collections::BTreeMap,
    net::SocketAddr,
    time::{Duration, Instant},
};

use anyhow::Context;
use async_trait::async_trait;
use futures_util::{StreamExt, TryStreamExt};
use lru::LruCache;
use melnet2::{
    wire::tcp::{Pipeline, TcpBackhaul},
    Backhaul, Swarm,
};
use novasmt::{CompressedProof, Database, InMemoryCas, Tree};
use parking_lot::Mutex;
use themelio_stf::SmtMapping;
use themelio_structs::{
    AbbrBlock, Address, Block, BlockHeight, ConsensusProof, NetID, Transaction, TxHash,
};

use melnet::MelnetError;
use smol::net::TcpListener;
use smol_timeout::TimeoutExt;
use themelio_nodeprot::{
    NodeClient, NodeResponder, NodeRpcClient, NodeRpcProtocol, NodeRpcService, NodeServer,
    StateSummary, Substate, TransactionError,
};
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
        storage: Storage,
        index: bool,
        swarm: Swarm<TcpBackhaul, NodeRpcClient<Pipeline>>,
    ) -> Self {
        // TODO:
        // 1. figure out if swarm::start_listen is all we need to do here?
        // 2. should we keep the old melnet as a separate network task?
        let _network_task = smolscale::spawn({
            async move {
                let swarm = swarm.clone();
                swarm
                    .start_listen(
                        listen_addr.to_string().into(),
                        advertise_addr.unwrap().to_string().into(),
                        NodeRpcService(NodeRpcImpl::new(
                            swarm.clone(),
                            netid,
                            storage.clone(),
                            index,
                        )),
                    )
                    .await;
            }
        });

        let _blksync_task = smolscale::spawn(blksync_loop(netid, swarm.clone(), storage));
        Self {
            _network_task,
            _blksync_task,
        }
    }
}

async fn blksync_loop(
    netid: NetID,
    swarm: Swarm<TcpBackhaul, NodeRpcClient<Pipeline>>,
    storage: Storage,
) {
    let tag = || format!("blksync@{:?}", storage.highest_state().header().height);
    loop {
        let gap_time: Duration = Duration::from_secs_f64(fastrand::f64() * 1.0);
        let routes = swarm.routes().await;
        let random_peer = routes.first().cloned();
        let backhaul = TcpBackhaul::new();
        if let Some(peer) = random_peer {
            log::trace!("picking peer {} out of {} peers", &peer, routes.len());
            let conn = backhaul.connect(peer.clone()).await.unwrap();
            let client = NodeRpcClient(conn);
            let addr: SocketAddr = peer.clone().to_string().parse().unwrap();
            let res = attempt_blksync(addr, &client, &storage).await;
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
    client: &NodeRpcClient<Pipeline>,
    storage: &Storage,
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
                    .context("timeout")
                    .ok()?;
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
        .try_buffered(32)
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
    storage: Storage,
    indexer: Option<BlockIndexer>,
    recent: Mutex<LruCache<TxHash, Instant>>,

    summary: Mutex<LruCache<BlockHeight, StateSummary>>,
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

        self.storage
            .mempool_mut()
            .apply_transaction(&tx)
            .map_err(|e| {
                if !e.to_string().contains("duplicate") {
                    log::warn!("cannot apply tx: {:?}", e);
                }
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
        let res = self
            .summary
            .lock()
            .get(&highest.inner_ref().height)
            .cloned();
        if let Some(res) = res {
            Ok(res)
        } else {
            let proof = self
                .storage
                .get_consensus(highest.inner_ref().height)
                .unwrap_or_default();
            let heh = StateSummary {
                netid: self.network,
                height: highest.inner_ref().height,
                header: highest.header(),
                proof,
            };
            self.summary
                .lock()
                .push(highest.inner_ref().height, heh.clone());
            Ok(heh)
        }
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
    fn new(network: NetID, storage: Storage, index: bool) -> Self {
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
            summary: LruCache::new(10).into(),
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

// NOTE: this struct is responsible for obtaining any "state" needed for the implementation of the RPC business logic.
pub struct NodeRpcImpl {
    network: NetID,
    storage: Storage,
    indexer: Option<BlockIndexer>,
    recent: Mutex<LruCache<TxHash, Instant>>,
    summary: Mutex<LruCache<BlockHeight, StateSummary>>,
    coin_smts: Mutex<LruCache<BlockHeight, Tree<InMemoryCas>>>,

    swarm: Swarm<TcpBackhaul, NodeRpcClient<Pipeline>>,
}

impl NodeRpcImpl {
    fn new(
        swarm: Swarm<TcpBackhaul, NodeRpcClient<Pipeline>>,
        network: NetID,
        storage: Storage,
        index: bool,
    ) -> Self {
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
            summary: LruCache::new(10).into(),
            swarm,
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

#[async_trait]
impl NodeRpcProtocol for NodeRpcImpl {
    async fn send_tx(&self, tx: Transaction) -> Result<(), TransactionError> {
        if let Some(val) = self.recent.lock().peek(&tx.hash_nosigs()) {
            if val.elapsed().as_secs_f64() < 10.0 {
                return Err(TransactionError::RecentlySeen);
            }
        }
        self.recent.lock().put(tx.hash_nosigs(), Instant::now());
        log::trace!("handling send_tx");
        let start = Instant::now();

        self.storage
            .mempool_mut()
            .apply_transaction(&tx)
            .map_err(|e| {
                if !e.to_string().contains("duplicate") {
                    log::warn!("cannot apply tx: {:?}", e);
                }
            })
            .or(Err(TransactionError::Storage));

        log::debug!(
            "txhash {}.. inserted ({:?} applying)",
            &tx.hash_nosigs().to_string()[..10],
            start.elapsed(),
        );

        // log::debug!("about to broadcast txhash {:?}", tx.hash_nosigs());
        let routes = self.swarm.routes().await;
        let backhaul = TcpBackhaul::new();
        for neigh in routes.iter().take(16).cloned() {
            let tx = tx.clone();
            let network = self.network;
            // log::debug!("bcast {:?} => {:?}", tx.hash_nosigs(), neigh);
            smolscale::spawn(async move {
                let conn = backhaul.connect(neigh).await.unwrap();
                NodeRpcClient(conn)
                    .send_tx(tx)
                    .timeout(Duration::from_secs(10))
                    .await
            })
            .detach();
        }

        Ok(())
    }

    async fn get_abbr_block(&self, height: BlockHeight) -> Option<(AbbrBlock, ConsensusProof)> {
        log::trace!("handling get_abbr_block({})", height);
        let state = self
            .storage
            .get_state(height)
            .context(format!("block {} not confirmed yet", height))
            .ok()?;
        let proof = self
            .storage
            .get_consensus(height)
            .context(format!("block {} not confirmed yet", height))
            .ok()?;
        Some((state.to_block().abbreviate(), proof))
    }

    async fn get_summary(&self) -> StateSummary {
        log::trace!("handling get_summary()");
        let highest = self.storage.highest_state();
        let res = self
            .summary
            .lock()
            .get(&highest.inner_ref().height)
            .cloned();
        if let Some(res) = res {
            res
        } else {
            let proof = self
                .storage
                .get_consensus(highest.inner_ref().height)
                .unwrap_or_default();
            let summary = StateSummary {
                netid: self.network,
                height: highest.inner_ref().height,
                header: highest.header(),
                proof,
            };
            self.summary
                .lock()
                .push(highest.inner_ref().height, summary.clone());
            summary
        }
    }

    async fn get_block(&self, height: BlockHeight) -> Option<Block> {
        log::trace!("handling get_state({})", height);
        Some(
            self.storage
                .get_state(height)
                .context("no such height")
                .ok()?
                .to_block(),
        )
    }

    async fn get_smt_branch(
        &self,
        height: BlockHeight,
        elem: Substate,
        key: HashVal,
    ) -> Option<(Vec<u8>, CompressedProof)> {
        log::trace!("handling get_smt_branch({}, {:?})", height, elem);
        let state = self
            .storage
            .get_state(height)
            .context(format!("block {} not confirmed yet", height))
            .ok()?;
        let ctree = self.get_coin_tree(height).ok()?;
        let (v, proof) = match elem {
            Substate::Coins => state.inner_ref().coins.inner().get_with_proof(key.0),
            Substate::History => state.inner_ref().history.mapping.get_with_proof(key.0),
            Substate::Pools => state.inner_ref().pools.mapping.get_with_proof(key.0),
            Substate::Stakes => state.inner_ref().stakes.mapping.get_with_proof(key.0),
            Substate::Transactions => ctree.get_with_proof(key.0),
        };
        Some((v.to_vec(), proof.compress()))
    }

    async fn get_stakers_raw(&self, height: BlockHeight) -> Option<BTreeMap<HashVal, Vec<u8>>> {
        let state = self
            .storage
            .get_state(height)
            .context("no such height")
            .ok()?;
        let mut accum = BTreeMap::new();
        for (k, v) in state.inner_ref().stakes.mapping.iter() {
            accum.insert(HashVal(k), v.to_vec());
        }
        Some(accum)
    }

    async fn get_some_coins(
        &self,
        height: BlockHeight,
        covhash: themelio_structs::Address,
    ) -> Option<Vec<themelio_structs::CoinID>> {
        self.indexer
            .as_ref()
            .and_then(|s| s.get(height).map(|idx| idx.lookup(covhash)))
    }
}
