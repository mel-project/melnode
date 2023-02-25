mod blksync;
mod indexer;

use crate::{node::blksync::attempt_blksync, storage::Storage};

use anyhow::Context;
use async_trait::async_trait;
use base64::Engine;
use futures_util::{StreamExt, TryStreamExt};
use lru::LruCache;
use melblkidx::{CoinInfo, Indexer};
use melnet2::{wire::http::HttpBackhaul, Backhaul, Swarm};
use novasmt::{CompressedProof, Database, InMemoryCas, Tree};
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use melstf::SmtMapping;
use melstructs::{
    AbbrBlock, Address, Block, BlockHeight, CoinID, ConsensusProof, NetID, Transaction, TxHash,
};
use std::{
    collections::BTreeMap,
    net::SocketAddr,
    time::{Duration, Instant},
};
use stdcode::StdcodeSerializeExt;

use melprot::{
    CoinChange, CoinSpendStatus, NodeRpcClient, NodeRpcProtocol, NodeRpcService, StateSummary,
    Substate, TransactionError,
};

use smol_timeout::TimeoutExt;
use tmelcrypt::{HashVal, Hashable};

use self::indexer::WrappedIndexer;

/// An actor implementing the node P2P protocol, common for both replicas and stakers..
pub struct Node {
    _blksync_task: smol::Task<()>,
}

impl Node {
    /// Creates a new Node.
    pub async fn start(
        netid: NetID,
        listen_addr: SocketAddr,

        advertise_addr: Option<SocketAddr>,
        storage: Storage,
        index_coins: bool,
        swarm: Swarm<HttpBackhaul, NodeRpcClient>,
    ) -> anyhow::Result<Self> {
        // This is all we need to do since start_listen does not block.
        log::debug!("starting to listen at {}", listen_addr);
        swarm
            .start_listen(
                listen_addr.to_string().into(),
                advertise_addr.map(|addr| addr.to_string().into()),
                NodeRpcService(
                    NodeRpcImpl::start(
                        swarm.clone(),
                        listen_addr,
                        netid,
                        storage.clone(),
                        index_coins,
                    )
                    .await?,
                ),
            )
            .await?;

        let _blksync_task = smolscale::spawn(blksync_loop(netid, swarm, storage));
        Ok(Self { _blksync_task })
    }
}

async fn blksync_loop(_netid: NetID, swarm: Swarm<HttpBackhaul, NodeRpcClient>, storage: Storage) {
    loop {
        let gap_time: Duration = Duration::from_secs_f64(fastrand::f64() * 1.0);
        let routes = swarm.routes().await;
        let random_peer = routes.first().cloned();
        if let Some(peer) = random_peer {
            log::trace!("picking peer {} out of {} peers", &peer, routes.len());
            let fallible_part = async {
                let client = swarm.connect(peer.clone()).await?;
                let addr: SocketAddr = peer.clone().to_string().parse()?;
                let res = attempt_blksync(addr, &client, &storage).await?;
                anyhow::Ok(res)
            };
            match fallible_part.await {
                Err(e) => {
                    log::warn!("failed to blksync with {}: {:?}", peer, e);
                    log::warn!("last state: {:?}", storage.highest_state().await.header());
                }
                Ok(blklen) => {
                    if blklen > 0 {
                        log::debug!("synced to height {:?}", storage.highest_height().await);
                    }
                }
            }
        }
        smol::Timer::after(gap_time).await;
    }
}

// This struct is responsible for obtaining any "state" needed for the implementation of the RPC business logic.
pub struct NodeRpcImpl {
    network: NetID,
    storage: Storage,
    recent: Mutex<LruCache<TxHash, Instant>>,
    summary: Mutex<LruCache<BlockHeight, StateSummary>>,
    coin_smts: Mutex<LruCache<BlockHeight, Tree<InMemoryCas>>>,
    abbr_block_cache: moka::sync::Cache<BlockHeight, (AbbrBlock, ConsensusProof)>,
    swarm: Swarm<HttpBackhaul, NodeRpcClient>,
    indexer: Option<WrappedIndexer>,
}

impl NodeRpcImpl {
    async fn start(
        swarm: Swarm<HttpBackhaul, NodeRpcClient>,
        listen_addr: SocketAddr,
        network: NetID,
        storage: Storage,
        index_coins: bool,
    ) -> anyhow::Result<Self> {
        let indexer = if index_coins {
            Some(WrappedIndexer::start(network, storage.clone(), listen_addr).await?)
        } else {
            None
        };
        Ok(Self {
            network,
            storage,
            recent: LruCache::new(1000).into(),
            coin_smts: LruCache::new(100).into(),
            summary: LruCache::new(10).into(),
            swarm,
            abbr_block_cache: moka::sync::Cache::new(100_000),
            indexer,
        })
    }

    async fn get_coin_tree(&self, height: BlockHeight) -> anyhow::Result<Tree<InMemoryCas>> {
        let otree = self.coin_smts.lock().get(&height).cloned();
        if let Some(v) = otree {
            Ok(v)
        } else {
            let state = self
                .storage
                .get_state(height)
                .await
                .context(format!("block {} not confirmed yet", height))?;
            let mut mm = SmtMapping::new(
                Database::new(InMemoryCas::default())
                    .get_tree(Default::default())
                    .unwrap(),
            );

            let transactions = state.to_block().transactions;
            for tx in transactions.iter() {
                mm.insert(tx.hash_nosigs(), tx.clone());
            }
            self.coin_smts.lock().put(height, mm.mapping.clone());
            Ok(mm.mapping)
        }
    }

    async fn get_indexer(&self) -> Option<&Indexer> {
        if let Some(indexer) = self.indexer.as_ref() {
            let indexer = indexer.inner();
            let height = self.storage.highest_height().await;
            while indexer.max_height() < height {
                log::warn!("waiting for {height} to be available at the indexer...");
                smol::Timer::after(Duration::from_secs(1)).await;
            }
            Some(indexer)
        } else {
            None
        }
    }
}

/// Global TCP backhaul for node connections
static TCP_BACKHAUL: Lazy<HttpBackhaul> = Lazy::new(HttpBackhaul::new);

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
                    log::warn!("cannot apply tx: {:?}", e)
                }
                TransactionError::Invalid(e.to_string())
            })?;

        log::debug!(
            "txhash {}.. inserted ({:?} applying)",
            &tx.hash_nosigs().to_string()[..10],
            start.elapsed(),
        );

        // log::debug!("about to broadcast txhash {:?}", tx.hash_nosigs());
        let routes = self.swarm.routes().await;
        for neigh in routes.iter().take(16).cloned() {
            let tx = tx.clone();
            smolscale::spawn(async move {
                let conn = TCP_BACKHAUL.connect(neigh).await?;
                NodeRpcClient(conn)
                    .send_tx(tx)
                    .timeout(Duration::from_secs(10))
                    .await
                    .context("oh no")???;
                anyhow::Ok(())
            })
            .detach();
        }

        Ok(())
    }

    async fn get_abbr_block(&self, height: BlockHeight) -> Option<(AbbrBlock, ConsensusProof)> {
        if let Some(c) = self.abbr_block_cache.get(&height) {
            return Some(c);
        }
        log::trace!("handling get_abbr_block({})", height);
        let block = self.storage.get_block(height).await?;
        let proof = self.storage.get_consensus(height).await?;
        let summ = (block.abbreviate(), proof);
        self.abbr_block_cache.insert(height, summ.clone());
        Some(summ)
    }

    async fn get_summary(&self) -> StateSummary {
        log::trace!("handling get_summary()");
        let highest = self.storage.highest_state().await;
        let header = highest.header();
        let res = self.summary.lock().get(&header.height).cloned();
        if let Some(res) = res {
            res
        } else {
            let proof = self
                .storage
                .get_consensus(header.height)
                .await
                .unwrap_or_default();
            let summary = StateSummary {
                netid: self.network,
                height: header.height,
                header,
                proof,
            };
            self.summary.lock().push(header.height, summary.clone());
            summary
        }
    }

    async fn get_block(&self, height: BlockHeight) -> Option<Block> {
        log::trace!("handling get_state({})", height);
        self.storage.get_block(height).await
    }

    async fn get_lz4_blocks(&self, height: BlockHeight, size_limit: usize) -> Option<String> {
        log::debug!("get_lz4_blocks({height}, {size_limit})");
        let size_limit = size_limit.min(10_000_000);
        // TODO: limit the *compressed* size. But this is fine because compression makes stuff smoller
        let mut total_count = 0;
        let mut accum = vec![];
        let mut proof_accum = vec![];

        let mut height = height;
        while total_count <= size_limit {
            if let Some(block) = self.get_block(height).await {
                match self.storage.get_consensus(height).await {
                    Some(proof) => {
                        total_count += block.stdcode().len();
                        total_count += proof.stdcode().len();

                        accum.push(block);
                        proof_accum.push(proof);

                        if total_count > size_limit {
                            log::info!("BATCH IS DONE");
                            if accum.len() > 1 {
                                accum.pop();
                            }
                        }

                        // only increment here
                        height += BlockHeight(1);
                    }
                    _ => {
                        log::warn!("no proof stored for height {}", height);
                    }
                }
            } else if accum.is_empty() {
                log::warn!("no stored block for height: {:?}", height);
                return None;
            } else {
                break;
            }
        }

        let compressed = lz4_flex::compress_prepend_size(&(accum, proof_accum).stdcode());
        Some(base64::engine::general_purpose::STANDARD_NO_PAD.encode(compressed))
    }

    async fn get_smt_branch(
        &self,
        height: BlockHeight,
        elem: Substate,
        key: HashVal,
    ) -> Option<(Vec<u8>, CompressedProof)> {
        log::trace!("handling get_smt_branch({}, {:?})", height, elem);
        let state = self.storage.get_state(height).await?;
        let ctree = self.get_coin_tree(height).await.ok()?;
        let coins_smt = state.raw_coins_smt();
        let history_smt = state.raw_history_smt();
        let pools_smt = state.raw_pools_smt();

        let (v, proof) = match elem {
            Substate::Coins => coins_smt.get_with_proof(key.0),
            Substate::History => history_smt.get_with_proof(key.0),
            Substate::Pools => pools_smt.get_with_proof(key.0),
            Substate::Stakes => todo!("no longer relevant"),
            Substate::Transactions => ctree.get_with_proof(key.0),
        };
        Some((v.to_vec(), proof.compress()))
    }

    async fn get_stakers_raw(&self, height: BlockHeight) -> Option<BTreeMap<HashVal, Vec<u8>>> {
        let state = self.storage.get_state(height).await?;
        // Note, the returned HashVal is >> HASHED AGAIN << because this is supposed to be compatible with the old SmtMapping encoding, where the key to the `stakes` SMT is the *hash of the transaction hash* due to a quirk.
        Some(
            state
                .raw_stakes()
                .iter()
                .map(|(k, v)| (k.0.hash(), v.stdcode()))
                .collect(),
        )
    }

    async fn get_some_coins(&self, height: BlockHeight, covhash: Address) -> Option<Vec<CoinID>> {
        let indexer = self.get_indexer().await?;
        let coins: Vec<CoinID> = indexer
            .query_coins()
            .covhash(covhash)
            .create_height_range(0..=height.0)
            .iter()
            .map(|c| CoinID {
                txhash: c.create_txhash,
                index: c.create_index,
            })
            .collect();
        Some(coins)
    }

    async fn get_coin_changes(
        &self,
        height: BlockHeight,
        covhash: Address,
    ) -> Option<Vec<CoinChange>> {
        log::debug!("get_coin_changes({height}, {covhash})");
        self.storage.get_block(height).await?;
        let indexer = self.get_indexer().await?;
        // get coins 1 block below the given height
        let deleted_coins: Vec<CoinInfo> = indexer
            .query_coins()
            .covhash(covhash)
            .spend_height_range(height.0..=height.0)
            .iter()
            .collect();

        // get coins at the given height
        let added_coins: Vec<CoinInfo> = indexer
            .query_coins()
            .covhash(covhash)
            .create_height_range(height.0..=height.0)
            .iter()
            .collect();

        if !added_coins.is_empty() || !deleted_coins.is_empty() {
            log::debug!(
                "{} added, {} deleted",
                added_coins.len(),
                deleted_coins.len()
            );
        }

        // which coins got added in after_coins?
        let added: Vec<CoinChange> = added_coins
            .iter()
            .map(|coin| CoinChange::Add(CoinID::new(coin.create_txhash, coin.create_index)))
            .collect();

        // which coins got deleted in before coins?
        let deleted: Vec<CoinChange> = deleted_coins
            .iter()
            .map(|coin| {
                CoinChange::Delete(
                    CoinID::new(coin.create_txhash, coin.create_index),
                    coin.spend_info.unwrap().spend_txhash,
                )
            })
            .collect();

        Some([added, deleted].concat())
    }

    async fn get_coin_spend(&self, coin: CoinID) -> Option<CoinSpendStatus> {
        let indexer = self.get_indexer().await?;

        let coin_info: Vec<CoinInfo> = indexer
            .query_coins()
            .create_txhash(coin.txhash)
            .create_index(coin.index)
            .iter()
            .collect();

        coin_info
            .first()
            .map(|coin| CoinSpendStatus::Spent((coin.create_txhash, coin.create_height)))
    }
}
