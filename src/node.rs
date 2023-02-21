use crate::storage::Storage;

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

use std::{
    collections::BTreeMap,
    net::SocketAddr,
    time::{Duration, Instant},
};
use stdcode::StdcodeSerializeExt;
use themelio_stf::SmtMapping;
use themelio_structs::{
    AbbrBlock, Address, Block, BlockHeight, Checkpoint, CoinID, ConsensusProof, NetID, Transaction,
    TxHash,
};

use melprot::{
    Client, CoinChange, CoinSpendStatus, NodeRpcClient, NodeRpcProtocol, NodeRpcService,
    StateSummary, Substate, TransactionError,
};

use smol_timeout::TimeoutExt;
use tmelcrypt::{HashVal, Hashable};

/// An actor implementing the node P2P protocol, common for both auditors and stakers..
pub struct Node {
    _blksync_task: smol::Task<()>,
}

impl Node {
    /// Creates a new Node.
    pub async fn new(
        netid: NetID,
        listen_addr: SocketAddr,
        legacy_listen_addr: Option<SocketAddr>,
        advertise_addr: Option<SocketAddr>,
        storage: Storage,
        index_coins: bool,
        swarm: Swarm<HttpBackhaul, NodeRpcClient>,
    ) -> Self {
        // This is all we need to do since start_listen does not block.
        log::debug!("starting to listen at {}", listen_addr);
        smol::future::block_on(
            swarm.start_listen(
                listen_addr.to_string().into(),
                advertise_addr.map(|addr| addr.to_string().into()),
                NodeRpcService(
                    NodeRpcImpl::new(
                        swarm.clone(),
                        listen_addr,
                        netid,
                        storage.clone(),
                        index_coins,
                    )
                    .await,
                ),
            ),
        )
        .expect("failed to start listening");

        let _blksync_task = smolscale::spawn(blksync_loop(netid, swarm, storage));
        Self { _blksync_task }
    }
}

fn netname(netid: NetID) -> &'static str {
    match netid {
        NetID::Mainnet => "mainnet-node",
        NetID::Testnet => "testnet-node",
        _ => Box::leak(Box::new(format!("{:?}", netid))),
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
                    log::warn!(
                        "last state: {:?}",
                        storage.highest_state().await.unwrap().header()
                    );
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

/// Attempts a sync using the given given node client.
async fn attempt_blksync(
    addr: SocketAddr,
    client: &NodeRpcClient,
    storage: &Storage,
) -> anyhow::Result<usize> {
    if std::env::var("MELNODE_OLD_BLKSYNC").is_ok() {
        return attempt_blksync_legacy(addr, client, storage).await;
    }
    log::debug!("starting blksync");
    let their_highest = client
        .get_summary()
        .timeout(Duration::from_secs(5))
        .await
        .context("timed out getting summary")?
        .context("cannot get their highest block")?
        .height;
    log::debug!("their_highest = {their_highest}");
    let my_highest = storage.highest_height().await?.unwrap_or_default();
    if their_highest <= my_highest {
        return Ok(0);
    }

    let mut num_blocks_applied: usize = 0;
    let my_highest: u64 = my_highest.0 + 1;

    let mut height = BlockHeight(my_highest);
    while height <= their_highest {
        let start = Instant::now();

        log::debug!("gonna get compressed blocks...");
        let compressed_blocks = client
            .get_lz4_blocks(height, 50_000)
            .timeout(Duration::from_secs(30))
            .await
            .context("timeout while getting compressed blocks")?
            .context("failed to get compressed blocks")?;
        log::debug!("got compressed blocks!");

        let (blocks, cproofs): (Vec<Block>, Vec<ConsensusProof>) = match compressed_blocks {
            Some(compressed) => {
                // decode base64 first
                let compressed_base64 = base64::engine::general_purpose::STANDARD_NO_PAD
                    .decode(compressed.as_bytes())?;

                // decompress
                let decompressed = lz4_flex::decompress_size_prepended(&compressed_base64)?;

                stdcode::deserialize::<(Vec<Block>, Vec<ConsensusProof>)>(&decompressed)?
            }
            _ => anyhow::bail!("missing block {height}"),
        };

        let mut last_applied_height = height;
        for (block, cproof) in blocks.iter().zip(cproofs) {
            // validate before applying
            if block.header.height != last_applied_height {
                anyhow::bail!("wanted block {}, but got {}", height, block.header.height);
            }
            log::debug!(
                "fully resolved block {} from peer {} in {:.2}ms",
                block.header.height,
                addr,
                start.elapsed().as_secs_f64() * 1000.0
            );

            storage
                .apply_block(block.clone(), cproof)
                .await
                .context("could not apply a resolved block")?;
            num_blocks_applied += 1;

            last_applied_height += BlockHeight(1);
            log::debug!("applied block {last_applied_height}");
        }

        height += BlockHeight(blocks.len() as u64);
    }

    Ok(num_blocks_applied)
}

/// Attempts a sync using the given given node client, in a legacy fashion.
async fn attempt_blksync_legacy(
    addr: SocketAddr,
    client: &NodeRpcClient,
    storage: &Storage,
) -> anyhow::Result<usize> {
    let their_highest = client
        .get_summary()
        .timeout(Duration::from_secs(5))
        .await
        .context("timed out getting summary")?
        .context("cannot get their highest block")?
        .height;
    let my_highest = storage.highest_height().await?.unwrap_or_default();
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

                let (block, cproof): (Block, ConsensusProof) = match client
                    .get_full_block(height, &lookup_tx)
                    .timeout(Duration::from_secs(15))
                    .await
                    .context("timeout")??
                {
                    Some(v) => v,
                    _ => anyhow::bail!("mysteriously missing block {}", height),
                };

                if block.header.height != height {
                    anyhow::bail!("WANTED BLK {}, got {}", height, block.header.height);
                }
                log::trace!(
                    "fully resolved block {} from peer {} in {:.2}ms",
                    block.header.height,
                    addr,
                    start.elapsed().as_secs_f64() * 1000.0
                );
                Ok((block, cproof))
            }))
        })
        .try_buffered(64)
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

// This struct is responsible for obtaining any "state" needed for the implementation of the RPC business logic.
pub struct NodeRpcImpl {
    network: NetID,
    storage: Storage,
    recent: Mutex<LruCache<TxHash, Instant>>,
    summary: Mutex<LruCache<BlockHeight, StateSummary>>,
    coin_smts: Mutex<LruCache<BlockHeight, Tree<InMemoryCas>>>,
    abbr_block_cache: moka::sync::Cache<BlockHeight, (AbbrBlock, ConsensusProof)>,
    swarm: Swarm<HttpBackhaul, NodeRpcClient>,
    indexer: Option<(Indexer, Client)>,
}

impl NodeRpcImpl {
    async fn new(
        swarm: Swarm<HttpBackhaul, NodeRpcClient>,
        listen_addr: SocketAddr,
        network: NetID,
        storage: Storage,
        index_coins: bool,
    ) -> Self {
        let indexer = if index_coins {
            let mut localhost_listen_addr = listen_addr;
            localhost_listen_addr.set_ip("127.0.0.1".parse().unwrap());
            // TODO: connect_lazy shouldn't return a Result, since backhaul.connect_lazy is "infallible"?
            let transport: NodeRpcClient = swarm
                .connect_lazy(localhost_listen_addr.to_string().into())
                .await
                .unwrap();
            let client = Client::new(network, transport);

            Some((
                Indexer::new(storage.get_indexer_path(), client.clone())
                    .expect("indexer failed to be created"),
                client,
            ))
        } else {
            None
        };
        Self {
            network,
            storage,
            recent: LruCache::new(1000).into(),
            coin_smts: LruCache::new(100).into(),
            summary: LruCache::new(10).into(),
            swarm,
            abbr_block_cache: moka::sync::Cache::new(100_000),
            indexer,
        }
    }

    async fn get_coin_tree(&self, height: BlockHeight) -> anyhow::Result<Tree<InMemoryCas>> {
        let otree = self.coin_smts.lock().get(&height).cloned();
        if let Some(v) = otree {
            Ok(v)
        } else {
            let state = self
                .storage
                .get_state(height)
                .await?
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
        let trusted_height = self
            .storage
            .get_state(BlockHeight(1))
            .await
            .expect("storage failed")?;
        let toret = self.indexer.as_ref().map(move |(indexer, client)| {
            client.trust(Checkpoint {
                height: BlockHeight(1),
                header_hash: trusted_height.header().hash(),
            });
            eprintln!("TRUSSSSSSST!!!!!!!");
            log::debug!("INDEXER OBTAINED");
            indexer
        });
        if let Some(indexer) = toret {
            let height = self
                .storage
                .highest_height()
                .await
                .unwrap()
                .unwrap_or_default();
            while indexer.max_height() < height {
                log::warn!("waiting for {height} to be available at the indexer...");
                smol::Timer::after(Duration::from_secs(1)).await;
            }
        }
        toret
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
        let block = self.storage.get_block(height).await.unwrap()?;
        let proof = self
            .storage
            .get_consensus(height)
            .await
            .unwrap()
            .context(format!("block {} not confirmed yet", height))
            .ok()?;
        let summ = (block.abbreviate(), proof);
        self.abbr_block_cache.insert(height, summ.clone());
        Some(summ)
    }

    async fn get_summary(&self) -> StateSummary {
        log::trace!("handling get_summary()");
        let highest = self.storage.highest_state().await.unwrap();
        let header = highest.header();
        let res = self.summary.lock().get(&header.height).cloned();
        if let Some(res) = res {
            res
        } else {
            let proof = self
                .storage
                .get_consensus(header.height)
                .await
                .unwrap()
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
        self.storage
            .get_block(height)
            .await
            .expect("could not read block")
    }

    async fn get_lz4_blocks(&self, height: BlockHeight, size_limit: usize) -> Option<String> {
        let size_limit = size_limit.min(10_000_000);
        // TODO: limit the *compressed* size. But this is fine because compression makes stuff smoller
        let mut total_count = 0;
        let mut accum = vec![];
        let mut proof_accum = vec![];

        let mut height = height;
        while total_count <= size_limit {
            if let Some(block) = self.get_block(height).await {
                match self.storage.get_consensus(height).await.unwrap() {
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
        let state = self
            .storage
            .get_state(height)
            .await
            .unwrap()
            .context(format!("block {} not confirmed yet", height))
            .ok()?;
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
        log::warn!("GETTING STAKERS FOR {height}");
        let state = self.storage.get_state(height).await.unwrap()?;
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
        if let Some(indexer) = self.get_indexer().await {
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
        } else {
            log::warn!("no coin indexer configured for current node");
            None
        }
    }

    async fn get_coin_changes(
        &self,
        height: BlockHeight,
        covhash: Address,
    ) -> Option<Vec<CoinChange>> {
        self.storage.get_block(height).await.unwrap()?;
        if let Some(indexer) = self.get_indexer().await {
            // get coins 1 block below the given height
            let before_coins: Vec<CoinInfo> = indexer
                .query_coins()
                .covhash(covhash)
                .unspent()
                .create_height_range(0..height.0)
                .iter()
                .collect();

            // get coins at the given height
            let after_coins: Vec<CoinInfo> = indexer
                .query_coins()
                .covhash(covhash)
                .unspent()
                .create_height_range(0..=height.0)
                .iter()
                .collect();

            // which coins got added in after_coins?
            let added: Vec<CoinChange> = after_coins
                .iter()
                .filter(|after| !before_coins.contains(after))
                .map(|coin| CoinChange::Add(CoinID::new(coin.create_txhash, coin.create_index)))
                .collect();

            // which coins got deleted in before coins?
            let deleted: Vec<CoinChange> = before_coins
                .iter()
                .filter(|before| !after_coins.contains(before))
                .map(|coin| {
                    CoinChange::Delete(
                        CoinID::new(coin.create_txhash, coin.create_index),
                        coin.spend_info.unwrap().spend_txhash,
                    )
                })
                .collect();

            Some([added, deleted].concat())
        } else {
            log::warn!("no coin indexer configured for current node");
            None
        }
    }

    async fn get_coin_spend(&self, coin: CoinID) -> Option<CoinSpendStatus> {
        if let Some(indexer) = self.get_indexer().await {
            let coin_info: Vec<CoinInfo> = indexer
                .query_coins()
                .create_txhash(coin.txhash)
                .create_index(coin.index)
                .iter()
                .collect();

            coin_info
                .first()
                .map(|coin| CoinSpendStatus::Spent((coin.create_txhash, coin.create_height)))
        } else {
            log::warn!("no coin indexer configured for current node");
            None
        }
    }
}
