use crate::{
    args::StakerConfig,
    storage::{MeshaCas, Storage},
};

use anyhow::Context;
use async_trait::async_trait;
use bytes::Bytes;

use dashmap::DashMap;
use melnet2::{wire::http::HttpBackhaul, Swarm};
use moka::sync::Cache;
use nanorpc::{nanorpc_derive, DynRpcTransport};

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use smol::{
    channel::{Receiver, Sender},
    prelude::*,
};
use smol_timeout::TimeoutExt;
use std::{collections::BTreeMap, time::Instant};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};
use stdcode::StdcodeSerializeExt;
use streamlette::{DeciderConfig, DiffMessage};
use tap::Tap;
use themelio_stf::SealedState;
use themelio_structs::{Block, BlockHeight, ConsensusProof, NetID, ProposerAction, StakeDoc};
use tmelcrypt::{Ed25519PK, Ed25519SK, HashVal};

static MAINNET_START_TIME: Lazy<SystemTime> = Lazy::new(|| {
    std::time::UNIX_EPOCH + Duration::from_secs(1618365600) + Duration::from_secs(30 * 7450)
}); // Apr 14 2021

static TESTNET_START_TIME: Lazy<SystemTime> =
    Lazy::new(|| std::time::UNIX_EPOCH + Duration::from_secs(1665123000));

fn height_to_time(netid: NetID, height: BlockHeight) -> SystemTime {
    match netid {
        NetID::Testnet => *TESTNET_START_TIME,
        NetID::Mainnet => *MAINNET_START_TIME,
        _ => SystemTime::now(),
    }
}

/// An actor that represents the background process that runs staker logic.
///
/// Talks to other stakers over the staker P2P network, decides on blocks using the Streamlette consensus algorithm, and stuffs decided blocks into [Storage].
pub struct Staker {
    _network_task: smol::Task<()>,
}

impl Staker {
    /// Creates a new instance of the staker protocol.
    pub fn new(storage: Storage, cfg: StakerConfig) -> Self {
        Self {
            _network_task: smolscale::spawn(network_task(storage, cfg)),
        }
    }
}

type DiffReq = (
    u128,
    HashMap<HashVal, HashVal>,
    async_oneshot::Sender<Vec<DiffMessage>>,
);

async fn network_task(storage: Storage, cfg: StakerConfig) {
    loop {
        if let Err(err) = network_task_inner(storage.clone(), cfg.clone()).await {
            log::warn!("staker failed: {:?}", err);
        }
    }
}

// TODO: the current consensus has a very small chance of not reaching completion, especially when the number of nodes is small compared to the network latency (e.g. a localhost simnet).
// This is because one could enter a situation where streamlette does decide, but less than 2/3 of the streamlette instances terminate properly, since the rest get stuck as 2/3 exit streamlette and no longer relay messages.
// This *seems* kinda fundamental to "oneshot" consensus that clean everything up after decision. I wonder whether there's a "standard solution" to this.

async fn network_task_inner(storage: Storage, cfg: StakerConfig) -> anyhow::Result<()> {
    // A channel for sending requests for diffs
    let (send_diff_req, recv_diff_req) = smol::channel::bounded::<DiffReq>(100);
    // The melnet2 swarm for the staker
    let swarm: Swarm<HttpBackhaul, StakerNetClient<DynRpcTransport>> = Swarm::new(
        HttpBackhaul::new(),
        |conn| StakerNetClient(DynRpcTransport::new(conn)),
        "melstaker-2",
    );
    swarm
        .add_route(cfg.bootstrap.to_string().into(), true)
        .await;
    // a "consensus proof gatherer" (see description)
    let sig_gather: Arc<ConsensusProofGatherer> = Arc::new(DashMap::new());
    swarm
        .start_listen(
            cfg.listen.to_string().into(),
            Some(cfg.listen.to_string().into()),
            StakerNetService(StakerNetProtocolImpl {
                send_diff_req,
                sig_gather: sig_gather.clone(),
                storage: storage.clone(),
            }),
        )
        .await
        .context("cannot start listen")?;
    // TODO better time calcs
    loop {
        let base_state = storage.highest_state().await;
        let next_height: BlockHeight = base_state.header().height + BlockHeight(1);
        let skip_round = async {
            storage.get_state_or_wait(next_height).await;
            log::warn!("skipping consensus for {next_height} since we already got it");
            anyhow::Ok(())
        };
        let next_time = height_to_time(base_state.header().network, next_height);
        if next_height.0 > 10 {
            sig_gather.remove(&BlockHeight(next_height.0 - 10));
        }

        while SystemTime::now() < next_time {
            smol::Timer::after(Duration::from_millis(100)).await;
        }

        log::debug!("starting consensus for {next_height}...");
        let consensus_start_time = Instant::now();

        let log_key = format!("{next_height}/{}", cfg.listen);

        let decide_round = async {
            let proposed_state = storage.mempool().to_state();
            let sealed_proposed_state = proposed_state.clone().seal(None);
            if sealed_proposed_state.header().height != next_height {
                log::warn!("mempool not at the right height, trying again");
                storage.mempool_mut().rebase(base_state);
            } else {
                let action = ProposerAction {
                    fee_multiplier_delta: if base_state.header().fee_multiplier
                        > cfg.target_fee_multiplier
                    {
                        -100
                    } else {
                        100
                    },
                    reward_dest: cfg.payout_addr,
                };
                // create the config
                let proposed_state = proposed_state.seal(Some(action));
                let config = StakerInner {
                    base_state: base_state.clone(),
                    my_proposal: proposed_state.to_block(),
                    // TODO: THIS MUST BE REPLACED WITH A PROPER MAJORITY BEACON FOR MANIPULATION RESISTANCE
                    nonce: base_state.header().height.0 as _,
                    my_sk: cfg.signing_secret,

                    recv_diff_req: recv_diff_req.clone(),
                    swarm: swarm.clone(),
                };
                let mut decider = streamlette::Decider::new(config);
                let decision = decider.tick_to_end().await;
                log::debug!(
                    "{log_key} DECIDED on a block with {} bytes within {:?}",
                    decision.len(),
                    consensus_start_time.elapsed()
                );
                let decision: Block = stdcode::deserialize(&decision)
                    .expect("decision reached on an INVALID block?!?!?!?!?!?!");

                // now we must assemble the consensus proof separately.
                // everybody has already decided on the block, we're just sharing signatures of it.

                // we start by inserting our own decision into the map.
                sig_gather.insert(
                    decision.header.height,
                    imbl::HashMap::new().tap_mut(|map| {
                        map.insert(
                            cfg.signing_secret.to_public(),
                            cfg.signing_secret.sign(&decision.header.hash()).into(),
                        );
                    }),
                );

                let _spammer = smolscale::spawn(async move { decider.sync_state(None).await });

                // then, until we finally have enough signatures, we spam our neighbors incessantly.
                let stakes = base_state.raw_stakes();
                let epoch = base_state.header().height.epoch();
                let vote_threshold = stakes.total_votes(epoch) * 2 / 3;
                let get_proof = || {
                    let map = sig_gather.entry(decision.header.height).or_default();
                    if map.keys().map(|pk| stakes.votes(epoch, *pk)).sum::<u128>() > vote_threshold
                    {
                        Some(map)
                    } else {
                        None
                    }
                };
                loop {
                    if let Some(result) = get_proof() {
                        let cproof: ConsensusProof =
                            result.clone().into_iter().map(|(k, v)| (k, v)).collect();
                        if let Err(err) = storage.apply_block(decision.clone(), cproof).await {
                            log::error!("cannot commit newly decided block: {:?}", err)
                        }
                        log::debug!(
                            "{log_key} COMMITTED the newly decided block within {:?}",
                            consensus_start_time.elapsed()
                        );
                        break;
                    }
                    let random_neigh = swarm.routes().await.first().cloned();
                    if let Some(neigh) = random_neigh {
                        log::trace!("syncing with {} for consensus proof", neigh);
                        let fallible = async {
                            let connection = swarm
                                .connect(neigh.clone())
                                .timeout(Duration::from_secs(1))
                                .await
                                .context("timed out for connection")??;
                            let result = connection
                                .get_sigs(next_height)
                                .timeout(Duration::from_secs(1))
                                .await
                                .context("timed out for getting")??;
                            anyhow::Ok(result)
                        };
                        match fallible.await {
                            Err(err) => log::warn!("cannot sync with {neigh}: {:?}", err),
                            Ok(map) => {
                                let mut existing = sig_gather.entry(next_height).or_default();
                                for (k, v) in map {
                                    existing.insert(k, v);
                                }
                                log::debug!(
                                    "{log_key}  now have {} votes in consensus proof after talking to {neigh}",
                                    existing.len()
                                );
                            }
                        }
                    }
                    smallsleep().await;
                }
            }
            anyhow::Ok(())
        };
        skip_round.or(decide_round).await?;
    }
}

struct StakerInner {
    base_state: SealedState<MeshaCas>,
    my_proposal: Block,
    nonce: u128,
    my_sk: Ed25519SK,

    recv_diff_req: Receiver<DiffReq>,
    swarm: Swarm<HttpBackhaul, StakerNetClient<DynRpcTransport>>,
}

#[async_trait]
impl DeciderConfig for StakerInner {
    fn generate_proposal(&self) -> Bytes {
        self.my_proposal.stdcode().into()
    }

    fn verify_proposal(&self, prop: &[u8]) -> bool {
        if let Ok(blk) = stdcode::deserialize::<Block>(prop) {
            self.base_state.apply_block(&blk).is_ok()
        } else {
            false
        }
    }

    async fn sync_core(&self, core: &mut streamlette::Core) {
        let core = RwLock::new(core);
        let main_loop = async {
            loop {
                let routes = self.swarm.routes().await;
                log::trace!("syncing core with {:?}", routes);
                for route in routes {
                    let our_summary = core.read().summary();
                    let fallible = async {
                        let conn = self
                            .swarm
                            .connect(route.clone())
                            .timeout(Duration::from_secs(1))
                            .await
                            .context("timed out connecting")??;
                        let diff: Vec<DiffMessage> = conn
                            .get_diff(self.nonce, our_summary.clone())
                            .timeout(Duration::from_secs(5))
                            .await
                            .context("timed out receiving diff")??;
                        anyhow::Ok(diff)
                    };
                    match fallible.await {
                        Ok(diff) => {
                            // apply the diffs
                            for diff in diff {
                                if let Err(err) = core.write().apply_one_diff(diff.clone()) {
                                    log::warn!("invalid diff from {route} ({:?}): {:?}", err, diff);
                                }
                            }
                        }
                        Err(err) => {
                            log::trace!("could not sync with {route}: {:?}", err)
                        }
                    }
                }
                smallsleep().await;
            }
        };
        let respond_loop = async {
            loop {
                if let Ok((nonce, their_summary, mut send_resp)) = self.recv_diff_req.recv().await {
                    if nonce == self.nonce {
                        let diff = core.read().get_diff(&their_summary);
                        let _ = send_resp.send(diff);
                    } else {
                        let _ = send_resp.send(vec![]);
                    }
                } else {
                    smol::future::pending::<()>().await;
                }
            }
        };
        main_loop.race(respond_loop).await
    }

    fn vote_weights(&self) -> BTreeMap<tmelcrypt::Ed25519PK, u64> {
        let height: BlockHeight = self.base_state.header().height + BlockHeight(1);
        self.base_state
            .raw_stakes()
            .pre_tip911()
            .iter()
            .fold(BTreeMap::new(), |mut map, val| {
                let stake_doc: StakeDoc = stdcode::deserialize(&val.1).unwrap();
                if height.epoch() >= stake_doc.e_start && height.epoch() < stake_doc.e_post_end {
                    *map.entry(stake_doc.pubkey).or_default() += stake_doc.syms_staked.0 as u64;
                }
                map
            })
    }

    fn seed(&self) -> u128 {
        self.nonce
    }

    fn my_secret(&self) -> Ed25519SK {
        self.my_sk
    }
}

async fn smallsleep() {
    smol::Timer::after(Duration::from_millis(fastrand::u64(100..200))).await;
}

#[nanorpc_derive]
#[async_trait]
pub trait StakerNetProtocol {
    /// Obtains a diff from the node, given a summary of the client's state.
    async fn get_diff(&self, nonce: u128, summary: HashMap<HashVal, HashVal>) -> Vec<DiffMessage>;
    /// Obtains all known signatures for the given confirmed height. Used to assemble [ConsensusProof]s after streamlette finishes deciding.
    async fn get_sigs(&self, height: BlockHeight) -> HashMap<Ed25519PK, Bytes>;
}

struct StakerNetProtocolImpl {
    send_diff_req: Sender<DiffReq>,
    sig_gather: Arc<ConsensusProofGatherer>,
    storage: Storage,
}

#[async_trait]
impl StakerNetProtocol for StakerNetProtocolImpl {
    async fn get_diff(&self, nonce: u128, summary: HashMap<HashVal, HashVal>) -> Vec<DiffMessage> {
        let (send_resp, recv_resp) = async_oneshot::oneshot();
        let _ = self.send_diff_req.try_send((nonce, summary, send_resp));

        if let Ok(val) = recv_resp.await {
            val
        } else {
            vec![]
        }
    }

    async fn get_sigs(&self, height: BlockHeight) -> HashMap<Ed25519PK, Bytes> {
        if let Some(val) = self.storage.get_consensus(height).await {
            val.into_iter().collect()
        } else {
            self.sig_gather
                .get(&height)
                .map(|s| s.clone())
                .unwrap_or_default()
                .into_iter()
                .collect() // convert from immutable to std
        }
    }
}

type ConsensusProofGatherer = DashMap<BlockHeight, imbl::HashMap<Ed25519PK, Bytes>>;
