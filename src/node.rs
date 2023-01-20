use crate::storage::Storage;

use std::{
    collections::BTreeMap,
    net::SocketAddr,
    time::{Duration, Instant},
};

use anyhow::Context;
use async_trait::async_trait;
use base64::Engine;
use futures_util::{StreamExt, TryStreamExt};
use lru::LruCache;
use melnet2::{wire::tcp::TcpBackhaul, Backhaul, Swarm};
use novasmt::{CompressedProof, Database, InMemoryCas, Tree};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use smol::net::TcpListener;
use stdcode::StdcodeSerializeExt;
use themelio_stf::SmtMapping;
use themelio_structs::{
    AbbrBlock, Address, Block, BlockHeight, ConsensusProof, NetID, Transaction, TxHash,
};

use smol_timeout::TimeoutExt;
use themelio_nodeprot::{
    CoinChange, NodeRpcClient, NodeRpcProtocol, NodeRpcService, StateSummary, Substate,
    TransactionError,
};
use tmelcrypt::HashVal;

type NrpcClient = NodeRpcClient<<TcpBackhaul as Backhaul>::RpcTransport>;

/// An actor implementing the node P2P protocol, common for both auditors and stakers..
pub struct Node {
    _blksync_task: smol::Task<()>,
    _legacy_task: Option<smol::Task<()>>,
}

impl Node {
    /// Creates a new Node.
    pub fn new(
        netid: NetID,
        listen_addr: SocketAddr,
        legacy_listen_addr: Option<SocketAddr>,
        advertise_addr: Option<SocketAddr>,
        storage: Storage,
        index: bool,
        swarm: Swarm<TcpBackhaul, NrpcClient>,
    ) -> Self {
        let _legacy_task = if let Some(legacy_listen_addr) = legacy_listen_addr {
            let network = melnet::NetState::new_with_name(netname(netid));
            network.listen(
                "node",
                NodeRpcService(NodeRpcImpl::new(
                    swarm.clone(),
                    netid,
                    storage.clone(),
                    index,
                )),
            );
            Some(smolscale::spawn({
                let network = network;
                async move {
                    let listener = TcpListener::bind(legacy_listen_addr).await.unwrap();
                    network.run_server(listener).await;
                }
            }))
        } else {
            None
        };

        // This is all we need to do since start_listen does not block.
        log::debug!("starting to listen at {}", listen_addr);
        smol::future::block_on(swarm.start_listen(
            listen_addr.to_string().into(),
            advertise_addr.map(|addr| addr.to_string().into()),
            NodeRpcService(NodeRpcImpl::new(
                swarm.clone(),
                netid,
                storage.clone(),
                index,
            )),
        ))
        .expect("failed to start listening");

        let _blksync_task = smolscale::spawn(blksync_loop(netid, swarm, storage));
        Self {
            _blksync_task,
            _legacy_task,
        }
    }
}

fn netname(netid: NetID) -> &'static str {
    match netid {
        NetID::Mainnet => "mainnet-node",
        NetID::Testnet => "testnet-node",
        _ => Box::leak(Box::new(format!("{:?}", netid))),
    }
}

async fn blksync_loop(_netid: NetID, swarm: Swarm<TcpBackhaul, NrpcClient>, storage: Storage) {
    loop {
        let gap_time: Duration = Duration::from_secs_f64(fastrand::f64() * 1.0);
        let routes = swarm.routes().await;
        let random_peer = routes.first().cloned();
        let backhaul = TcpBackhaul::new();
        if let Some(peer) = random_peer {
            log::trace!("picking peer {} out of {} peers", &peer, routes.len());
            let fallible_part = async {
                let conn = backhaul.connect(peer.clone()).await?;
                let client = NodeRpcClient(conn);
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
    client: &NrpcClient,
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
            std::env::var("MELNODE_BLKSYNC_BATCH")
                .ok()
                .and_then(|d| d.parse().ok())
                .unwrap_or(1000),
        );

    let mut result_stream = height_stream
        .map(Ok::<_, anyhow::Error>)
        .try_filter_map(|height| async move {
            Ok(Some(async move {
                let start = Instant::now();

                // get the compressed blocks
                let compressed_blocks = client
                    .get_lz4_blocks(height, 10_000_000)
                    .await
                    .context("failed to get compressed blocks")?;

                let (blocks, cproofs): (Vec<Block>, Vec<ConsensusProof>) = match compressed_blocks {
                    Some(compressed) => {
                        // decode base64 first
                        let compressed_base64 = base64::engine::general_purpose::STANDARD_NO_PAD
                            .decode(compressed.as_bytes())?;

                        // decompress
                        let decompressed = lz4_flex::decompress_size_prepended(&compressed_base64)?;
                        let (blocks, cproofs) = stdcode::deserialize::<(
                            Vec<Block>,
                            Vec<ConsensusProof>,
                        )>(&decompressed)?;

                        // validate before returning
                        for block in blocks.iter() {
                            if block.header.height != height {
                                anyhow::bail!("WANTED BLK {}, got {}", height, block.header.height);
                            }

                            log::trace!(
                                "fully resolved block {} from peer {} in {:.2}ms",
                                block.header.height,
                                addr,
                                start.elapsed().as_secs_f64() * 1000.0
                            );
                        }

                        (blocks, cproofs)
                    }
                    _ => anyhow::bail!("missing block {height}"),
                };
                // return the tuple
                Ok((blocks, cproofs))
            }))
        })
        .try_buffered(64)
        .boxed();

    // apply the blocks
    let mut num_blocks_applied = 0;
    while let Some(res) = result_stream.try_next().await? {
        let (blocks, cproofs): (Vec<Block>, Vec<ConsensusProof>) = res;
        for (block, cproof) in blocks.iter().zip(cproofs) {
            storage
                .apply_block(block.clone(), cproof)
                .await
                .context("could not apply a resolved block")?;
            num_blocks_applied += 1;
        }
    }
    Ok(num_blocks_applied)
}

// NOTE: this struct is responsible for obtaining any "state" needed for the implementation of the RPC business logic.
pub struct NodeRpcImpl {
    network: NetID,
    storage: Storage,

    recent: Mutex<LruCache<TxHash, Instant>>,
    summary: Mutex<LruCache<BlockHeight, StateSummary>>,
    coin_smts: Mutex<LruCache<BlockHeight, Tree<InMemoryCas>>>,

    abbr_block_cache: moka::sync::Cache<BlockHeight, (AbbrBlock, ConsensusProof)>,

    swarm: Swarm<TcpBackhaul, NrpcClient>,
}

impl NodeRpcImpl {
    fn new(
        swarm: Swarm<TcpBackhaul, NrpcClient>,
        network: NetID,
        storage: Storage,
        _index: bool,
    ) -> Self {
        Self {
            network,
            storage,

            recent: LruCache::new(1000).into(),
            coin_smts: LruCache::new(100).into(),
            summary: LruCache::new(10).into(),
            swarm,

            abbr_block_cache: moka::sync::Cache::new(100_000),
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
}

/// Global TCP backhaul for node connections
static TCP_BACKHAUL: Lazy<TcpBackhaul> = Lazy::new(TcpBackhaul::new);

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
        let state = self
            .storage
            .get_state(height)
            .await
            .unwrap()
            .context(format!("block {} not confirmed yet", height))
            .ok()?;
        let proof = self
            .storage
            .get_consensus(height)
            .await
            .unwrap()
            .context(format!("block {} not confirmed yet", height))
            .ok()?;
        let summ = (state.to_block().abbreviate(), proof);
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
        Some(
            self.storage
                .get_state(height)
                .await
                .unwrap()
                .context("no such height")
                .ok()?
                .to_block(),
        )
    }

    async fn get_lz4_blocks(&self, height: BlockHeight, size_limit: usize) -> Option<String> {
        let size_limit = size_limit.min(10_000_000);
        // TODO limit the *compressed* size. But this is fine because compression makes stuff smoller
        let mut total_count = 0;
        let mut accum: Vec<Block> = vec![];
        let mut proof_accum: Vec<ConsensusProof> = vec![];
        for height in height.0.. {
            let height = BlockHeight(height);
            if let Some(block) = self.get_block(height).await {
                if let Some(proof) = self.get_abbr_block(height).await.map(|s| s.1) {
                    total_count += block.stdcode().len();
                    total_count += proof.stdcode().len();
                    accum.push(block);
                    proof_accum.push(proof);

                    if total_count > size_limit {
                        if accum.len() > 1 {
                            accum.pop();
                        }
                        break;
                    }
                }
            } else {
                if accum.is_empty() {
                    return None;
                }
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

    async fn get_stakers_raw(&self, _height: BlockHeight) -> Option<BTreeMap<HashVal, Vec<u8>>> {
        todo!("no longer relevant")
    }

    async fn get_some_coins(
        &self,
        _height: BlockHeight,
        _covhash: themelio_structs::Address,
    ) -> Option<Vec<themelio_structs::CoinID>> {
        None
    }

    async fn get_coin_changes(&self, height: BlockHeight, address: Address) -> Vec<CoinChange> {
        todo!("fill in after the internal coin indexing moves to use the one from melscan")
    }
}
