use crate::blockgraph::{BlockGraph, BlockGraphDiff, Proposal, ProposalRejection};
use crate::{
    msg::{ProposalSig, VoteSig},
    NS_EXECUTOR,
};
use binary_search::{binary_search, Direction};
use dashmap::DashMap;
use melnet::{MelnetError, Request};
use novasmt::ContentAddrStore;
use parking_lot::RwLock;
use smol::channel::Receiver;
use smol::channel::Sender;
use smol_timeout::TimeoutExt;
use std::{
    collections::BTreeMap,
    convert::TryInto,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime},
};
use themelio_stf::{tip_heights::TIP_906_HEIGHT, ConfirmedState, SealedState};
use themelio_structs::{Block, BlockHeight, ProposerAction, Transaction, TxHash, STAKE_EPOCH};
use thiserror::Error;

use tmelcrypt::{Ed25519PK, Ed25519SK, HashVal};

/// A trait that represents a "mempool".
pub trait BlockBuilder<C: ContentAddrStore>: 'static + Send + Sync {
    /// Given a previous state, build a block that extends it
    fn build_block(&self, tip: SealedState<C>) -> Block;

    /// Sets a "hint" that the next block will extend from a particular state.
    #[allow(unused_variables)]
    fn hint_next_build(&self, tip: SealedState<C>) {}

    /// Gets a cached transaction if available.
    #[allow(unused_variables)]
    fn get_cached_transaction(&self, txhash: TxHash) -> Option<Transaction> {
        None
    }
}

/// Configuration for a running protocol.
pub struct EpochConfig<B: BlockBuilder<C>, C: ContentAddrStore> {
    pub listen: SocketAddr,
    pub bootstrap: Vec<SocketAddr>,
    pub genesis: SealedState<C>,
    pub start_time: SystemTime,
    pub interval: Duration,
    pub signing_sk: Ed25519SK,
    pub builder: B,
    pub get_confirmed:
        Box<dyn Fn(BlockHeight) -> Option<ConfirmedState<C>> + Sync + Send + 'static>,
}

/// Represents a running instance of the Symphonia protocol for a particular epoch.
pub struct EpochProtocol<C: ContentAddrStore> {
    _task: smol::Task<()>,
    cstate: Arc<RwLock<BlockGraph<C>>>,
    recv_confirmed: Receiver<ConfirmedState<C>>,
}

impl<C: ContentAddrStore> EpochProtocol<C> {
    /// Create a new instance of the protocol over melnet.
    pub fn new<B: BlockBuilder<C>>(cfg: EpochConfig<B, C>) -> Self {
        let (send_confirmed, recv_confirmed) = smol::channel::unbounded();

        let stake_map = &cfg.genesis.inner_ref().stakes;
        let vote_weights = stake_map
            .val_iter()
            .map(|stakedoc| {
                (
                    stakedoc.pubkey,
                    stake_map.vote_power(cfg.genesis.inner_ref().height.epoch(), stakedoc.pubkey),
                )
            })
            .collect::<BTreeMap<_, _>>();

        let blockgraph = BlockGraph::new(cfg.genesis.clone(), vote_weights);
        let cstate = Arc::new(RwLock::new(blockgraph));
        Self {
            _task: {
                let cstate = cstate.clone();
                NS_EXECUTOR.spawn(async move {
                    protocol_loop(cfg, cstate, send_confirmed).await;
                })
            },
            cstate,
            recv_confirmed,
        }
    }

    /// Receives the next fully-confirmed state.
    pub async fn next_confirmed(&self) -> ConfirmedState<C> {
        self.recv_confirmed.recv().await.unwrap()
    }

    /// Forces the given state to be genesis.
    pub fn reset_genesis(&self, genesis: SealedState<C>) {
        self.cstate.write().update_root(genesis)
    }
}

#[derive(Error, Debug)]
enum ProtocolError {
    #[error("melnet error: {0}")]
    Melnet(melnet::MelnetError),
    #[error("proposal rejection: {0}")]
    Proposal(crate::blockgraph::ProposalRejection),
    #[error("custom protocol error: {0}")]
    Custom(String),
}

impl From<ProposalRejection> for ProtocolError {
    fn from(e: ProposalRejection) -> Self {
        ProtocolError::Proposal(e)
    }
}

impl From<MelnetError> for ProtocolError {
    fn from(e: MelnetError) -> Self {
        ProtocolError::Melnet(e)
    }
}

async fn gossip_and_add_diff<C: ContentAddrStore>(
    cstate: Arc<RwLock<BlockGraph<C>>>,
    network: &melnet::NetState,
    voter_key: Ed25519SK,
) -> Result<(), ProtocolError> {
    // Send a summary to a random peer
    let summary = cstate.read().summarize();
    if let Some(rnd_peer) = network.routes().get(0) {
        let diff = melnet::request::<_, Vec<BlockGraphDiff>>(
            *rnd_peer,
            "symphgossip",
            "get_diff",
            summary,
        )
        .timeout(Duration::from_secs(10))
        //.map_err(|e| log::warn!("gossip request failed with peer {rnd_peer}: {e}"))
        .await
        .expect("timeout error?!")?;

        // Integrate diff into block graph
        cstate.write().merge_diff(diff)?;

        // Vote for all lnc tips
        cstate.write().vote_all(voter_key);

        Ok(())
    } else {
        Err(ProtocolError::Custom("Failed to get a peer".into()))
    }
}

/// Communicate summaries to peers and integrate diff responses into the chain state
async fn graph_gossip<C: ContentAddrStore>(
    epoch: u64,
    cstate: Arc<RwLock<BlockGraph<C>>>,
    network: melnet::NetState,
    send_confirmed: Sender<ConfirmedState<C>>,
    voter_key: Ed25519SK,
) -> ! {
    let cstate_inner = cstate.clone();
    network.listen(
        "get_diff",
        move |breq: Request<crate::blockgraph::Summary>| {
            let cstate_inner = cstate_inner.clone();
            async move {
                let response = cstate_inner.read().partial_summary_diff(&breq.body);
                Ok(response)
            }
        },
    );
    let (send_finalized, recv_finalized) = smol::channel::bounded(1);
    let _confirm_gossip = NS_EXECUTOR.spawn(confirm_gossip(
        epoch,
        cstate.clone(),
        network.clone(),
        recv_finalized,
        send_confirmed,
        voter_key,
    ));
    loop {
        // Get a blockgraph update from a random neighbor
        if let Err(err) = gossip_and_add_diff(cstate.clone(), &network, voter_key).await {
            log::warn!("error in gossip_and_add_diff: {:?}", err);
        }

        // Drain any new finalized blocks
        let finalized = cstate.write().drain_finalized();
        for block in finalized {
            log::debug!("Block finalized: {:?}", block.header());
            send_finalized
                .send(block)
                .await
                .expect("Failed to send a block on finalized channel");
        }

        smol::Timer::after(Duration::from_millis(100)).await;
    }
}

/// Gather confirmations of fully confirmed blocks from peers
async fn confirm_gossip<C: ContentAddrStore>(
    epoch: u64,
    cstate: Arc<RwLock<BlockGraph<C>>>,
    network: melnet::NetState,
    recv_finalized: Receiver<SealedState<C>>,
    send_confirmed: Sender<ConfirmedState<C>>,
    signing_sk: Ed25519SK,
) {
    // Cache of confirmation votes, indexed by block
    let confirmation_cache: Arc<DashMap<BlockHeight, BTreeMap<Ed25519PK, Vec<u8>>>> =
        Arc::new(DashMap::new());
    network.listen("get_confirmations", {
        let confirmation_cache = confirmation_cache.clone();
        move |breq: Request<BlockHeight>| {
            let confirmation_cache = confirmation_cache.clone();
            async move {
                let response = confirmation_cache
                    .get(&breq.body)
                    .map(|d| d.clone())
                    .unwrap_or_default();
                Ok(response)
            }
        }
    });
    // For every finalized block, we first vote for it, and then spawn a task that gathers confirmations until enough is gathered to confirm the block
    loop {
        let finalized = match recv_finalized.recv().await {
            Ok(s) => s,
            Err(err) => {
                log::warn!("confirm_gossip dying from bad recv: {}", err);
                return;
            }
        };
        if finalized.inner_ref().height.epoch() > epoch {
            log::warn!("stopping all confirmations because we are past epoch");
            return;
        }
        let mut mapping = BTreeMap::new();
        mapping.insert(
            signing_sk.to_public(),
            signing_sk.sign(&finalized.header().hash()),
        );
        let fin_height = finalized.inner_ref().height;
        confirmation_cache.insert(fin_height, mapping);
        // confirm it by randomly asking peers
        while confirmation_cache
            .get(&fin_height)
            .unwrap()
            .iter()
            .map(|(k, _)| cstate.read().vote_weight(*k))
            .sum::<f64>()
            <= 0.667
        {
            for peer in network.routes() {
                let ccache = confirmation_cache.clone();
                NS_EXECUTOR
                    .spawn(async move {
                        let their_mapping: BTreeMap<Ed25519PK, Vec<u8>> = match melnet::request(
                            peer,
                            "symphgossip",
                            "get_confirmations",
                            fin_height,
                        )
                        .await
                        {
                            Ok(r) => r,
                            Err(err) => {
                                log::warn!("error getting confirmation from {}: {:?}", peer, err);
                                return;
                            }
                        };
                        for (k, v) in their_mapping {
                            if let Some(mut m) = ccache.get_mut(&fin_height) {
                                m.insert(k, v);
                            }
                        }
                    })
                    .detach();
                smol::Timer::after(Duration::from_millis(200)).await;
            }
            smol::Timer::after(Duration::from_millis(200)).await;
        }
        log::debug!("CONFIRMED block {}", fin_height);
        let _ = send_confirmed
            .send(
                finalized
                    .confirm(confirmation_cache.get(&fin_height).unwrap().clone(), None)
                    .unwrap(),
            )
            .await;
    }
}

async fn protocol_loop<B: BlockBuilder<C>, C: ContentAddrStore>(
    cfg: EpochConfig<B, C>,
    cstate: Arc<RwLock<BlockGraph<C>>>,
    send_confirmed: Sender<ConfirmedState<C>>,
) {
    let cfg = Arc::new(cfg);
    let height_to_proposer = gen_get_proposer(cfg.genesis.clone());
    let network = melnet::NetState::new_with_name("symphgossip");
    for addr in &cfg.bootstrap {
        network.add_route(*addr);
    }

    let my_epoch = (cfg.genesis.inner_ref().height + 1.into()).epoch();

    // Spawn gossip loop
    let _gossiper = NS_EXECUTOR.spawn(graph_gossip(
        my_epoch,
        cstate.clone(),
        network.clone(),
        send_confirmed,
        cfg.signing_sk,
    ));

    // Run melnet instance in the background
    network.add_route(cfg.listen);
    let listener = smol::net::TcpListener::bind(cfg.listen)
        .await
        .expect("could not start to listen");
    let net_inner = network.clone();
    let _server = NS_EXECUTOR.spawn(async move { net_inner.run_server(listener).await });

    loop {
        let lnc_state = cstate
            .read()
            .lnc_state()
            .unwrap_or_else(|| cstate.read().root());

        let (height, height_time) =
            next_height_time(lnc_state.inner_ref().height, cfg.start_time, cfg.interval);
        //log::debug!("waiting until height_time {:?}", height_time);
        wait_until_sys(height_time).await;

        //log::debug!("entering height {}", height);

        if height_to_proposer(height) == cfg.signing_sk.to_public() {
            log::debug!("we are the proposer for height {}", height);

            let lnc_state = cstate
                .read()
                .lnc_state()
                .unwrap_or_else(|| cstate.read().root());
            let mut build_upon = lnc_state;
            if build_upon.inner_ref().height >= height {
                log::warn!(
                    "already have height {} > {}, skipping this round",
                    build_upon.inner_ref().height,
                    height
                );
                continue;
            }
            let last_nonempty_hash = build_upon.header().hash();
            // fill in a bunch of empty blocks until the height matches
            log::debug!("Stemming from {:?}", last_nonempty_hash);
            while build_upon.inner_ref().height + BlockHeight(1) < height {
                build_upon = smol::unblock(move || build_upon.next_state().seal(None)).await;
                log::debug!("building empty block for {}", build_upon.inner_ref().height);
            }

            // am i out of bounds?
            let out_of_bounds = (build_upon.inner_ref().height + 1.into()).epoch() > my_epoch;
            if out_of_bounds {
                log::warn!(
                    "novasymph running out of bounds: {} is out of epoch {}",
                    build_upon.inner_ref().height + 1.into(),
                    my_epoch
                )
            };

            // Propose an empty block with no reward if out of bounds
            let proposed_block = Arc::new(if out_of_bounds {
                build_upon
                    .next_state()
                    .seal(Some(ProposerAction {
                        fee_multiplier_delta: 0,
                        reward_dest: HashVal::default().into(),
                    }))
                    .to_block()
            } else {
                cfg.builder.build_block(build_upon.clone())
            });
            log::debug!("Proposing block {:?}", proposed_block.header.hash());

            // Insert proposal into blockgraph
            if let Err(err) = cstate.write().insert_proposal(Proposal {
                extends_from: last_nonempty_hash,
                block: proposed_block.clone(),
                proposer: cfg.signing_sk.to_public(),
                signature: ProposalSig::generate(cfg.signing_sk, &proposed_block.abbreviate()),
            }) {
                log::error!("***** OH MY GOD VERY FATAL ERROR (issue #27) *****");
                log::error!("Error: {:?}", err);
                log::error!(
                    "while building upon {} with block hash {} with {} txx, last_nonempty {}",
                    build_upon.header().hash(),
                    proposed_block.header.hash(),
                    proposed_block.transactions.len(),
                    last_nonempty_hash
                );
                log::error!(
                    "did I fail again? {}",
                    build_upon.apply_block(&proposed_block).is_err()
                );
                log::error!("proposer action: {:?}", proposed_block.proposer_action);
                for _ in 0..10 {
                    let mut build_upon_state = build_upon.next_state();
                    build_upon_state
                        .apply_tx_batch(
                            &proposed_block
                                .transactions
                                .iter()
                                .cloned()
                                .collect::<Vec<_>>(),
                        )
                        .unwrap();
                    log::error!(
                        "possible coins hash: {}",
                        build_upon_state
                            .seal(proposed_block.proposer_action)
                            .inner_ref()
                            .coins
                            .root_hash()
                    )
                }
                panic!("PANIK PANIK");
            }
            // vote for it myself
            //cstate.write().blockgraph.vote_all(cfg.signing_sk);
            cstate.write().insert_vote(
                proposed_block.header.hash(),
                cfg.signing_sk.to_public(),
                VoteSig::generate(cfg.signing_sk, proposed_block.header.hash()),
            );
            for state in dbg!(cstate.read().lnc_tips()) {
                log::debug!("lnc tip: {:?}", state)
            }
        } else {
            //log::debug!("we are NOT the proposer for height {}", height);
        }
    }
}

async fn wait_until_sys(sys: SystemTime) {
    let now = SystemTime::now();
    if sys < now {
    } else {
        let interv = sys.duration_since(now).unwrap();
        smol::Timer::after(interv).await;
    }
}

/// waits until the next block height, then returns that height
fn next_height_time(
    current_height: BlockHeight,
    start_time: SystemTime,
    interval: Duration,
) -> (BlockHeight, SystemTime) {
    let now = SystemTime::now();
    let next_height = time_to_height(start_time, interval, now) + BlockHeight(1);
    let next_time = height_to_time(start_time, interval, next_height);
    (next_height, next_time)
}

fn height_to_time(start_time: SystemTime, interval: Duration, height: BlockHeight) -> SystemTime {
    let normal = (interval * (height.0 as u32)).as_secs_f64();
    let smeared = normal + 600.0;
    if height >= TIP_906_HEIGHT {
        start_time + Duration::from_secs_f64(smeared)
    } else {
        start_time + Duration::from_secs_f64(normal)
    }
}

fn time_to_height(start_time: SystemTime, interval: Duration, time: SystemTime) -> BlockHeight {
    binary_search((0, ()), (1u64 << 28, ()), |h| {
        if height_to_time(start_time, interval, BlockHeight(h)) < time {
            Direction::Low(())
        } else {
            Direction::High(())
        }
    })
    .0
     .0
    .into()
}

// a helper function that returns a proposer-calculator for a given epoch
pub fn gen_get_proposer<C: ContentAddrStore>(
    //pub async fn gen_get_proposer<C: ContentAddrStore>(
    genesis: SealedState<C>,
) -> impl Fn(BlockHeight) -> Ed25519PK {
    let end_height = if genesis.inner_ref().height.epoch() == 0 {
        BlockHeight(0)
    } else if genesis.inner_ref().height.epoch() != (genesis.inner_ref().height + 1.into()).epoch()
    {
        genesis.inner_ref().height
    } else {
        BlockHeight((genesis.inner_ref().height.0 / STAKE_EPOCH * STAKE_EPOCH) - 1)
    };
    if end_height > BlockHeight(0) {
        assert!(end_height.0 % STAKE_EPOCH == STAKE_EPOCH - 1)
    }
    // majority beacon of all the blocks in the previous epoch
    let beacon_components = {
        let genesis = genesis.clone();
        //smol::unblock(move || {
        if end_height.0 >= STAKE_EPOCH {
            (end_height.0 - STAKE_EPOCH..end_height.0)
                .filter_map(|height| {
                    if height % 197 != 0 {
                        None
                    } else {
                        log::warn!("majority beacon looking at height {}", height);
                        Some(
                            genesis
                                .inner_ref()
                                .history
                                .get(&BlockHeight(height))
                                .0
                                .expect("getting history failed")
                                .hash(),
                        )
                    }
                })
                // .chain(std::iter::once(genesis.header().hash()))
                .collect::<Vec<_>>()
        } else {
            vec![HashVal::default()]
        }
        //})
    };
    //.await;
    let epoch = (genesis.inner_ref().height + BlockHeight(1)).epoch();
    let seed = tmelcrypt::majority_beacon(&beacon_components);
    let stakes = genesis.inner_ref().stakes.clone();

    dbg!(stakes.val_iter().collect::<Vec<_>>());
    dbg!(genesis.inner_ref().height);

    move |height: BlockHeight| {
        // we sum the number of µsyms staked
        // TODO: overflow?
        let total_staked = stakes
            .val_iter()
            .filter_map(|v| {
                if v.e_post_end > epoch && v.e_start <= epoch {
                    Some(v.syms_staked.0)
                } else {
                    None
                }
            })
            .sum::<u128>();
        if total_staked == 0 {
            panic!(
                "BLOCK {} (epoch {}, pre_epoch {}) DOES NOT HAVE STAKERS",
                height,
                epoch,
                genesis.inner_ref().height
            );
        }
        //log::debug!("{} staked for epoch {}", total_staked, epoch);
        // "clamp" the subseed
        // we hash the seed with the height
        let mut seed = tmelcrypt::hash_keyed(&height.0.to_be_bytes(), &seed);
        let seed = loop {
            let numseed = u128::from_be_bytes(
                (&tmelcrypt::hash_keyed(&height.0.to_be_bytes(), &seed).0[0..16])
                    .try_into()
                    .unwrap(),
            );
            let numseed = numseed >> total_staked.leading_zeros();
            if numseed <= total_staked {
                break numseed;
            }
            seed = tmelcrypt::hash_single(&seed);
        };
        // now we go through the stakedocs
        let mut stake_docs = stakes.val_iter().collect::<Vec<_>>();
        stake_docs.sort_by_key(|v| v.pubkey);
        let mut sum = 0;
        for stake in stake_docs {
            if stake.e_post_end > epoch && stake.e_start <= epoch {
                sum += stake.syms_staked.0;
                //dbg!(seed, sum);
                if seed <= sum {
                    return stake.pubkey;
                }
            }
        }
        unreachable!()
    }
}
