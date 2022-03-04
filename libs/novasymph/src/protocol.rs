use crate::{
    cstate::{
        gossip::{
            AbbrBlockResponse, BlockRequest, FullBlockResponse, TransactionRequest,
            TransactionResponse,
        },
        ChainState,
    },
    msg::ProposalSig,
    NS_EXECUTOR,
};
use thiserror::Error;
use melnet::{Request, MelnetError};
use crate::blockgraph::{BlockGraph, ProposalRejection, BlockGraphDiff};
use binary_search::{binary_search, Direction};
use derivative::Derivative;
use futures_util::future::TryFutureExt;
use futures_util::stream::FuturesOrdered;
use novasmt::ContentAddrStore;
use parking_lot::RwLock;
use smol::{channel::Receiver, future::Boxed};
use smol::{channel::Sender, prelude::*};
use smol_timeout::TimeoutExt;
use std::{
    collections::{BTreeSet, BTreeMap, HashSet},
    convert::TryInto,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime},
};
use themelio_stf::{tip_heights::TIP_906_HEIGHT, ConfirmedState, SealedState, StakeMapping};
use themelio_structs::{
    Block, BlockHeight, ConsensusProof, ProposerAction, Transaction, TxHash, STAKE_EPOCH,
};

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
    pub forest: novasmt::Database<C>,
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
    cstate: Arc<RwLock<ChainState<C>>>,
    recv_confirmed: Receiver<ConfirmedState<C>>,
}

impl<C: ContentAddrStore> EpochProtocol<C> {
    /// Create a new instance of the protocol over melnet.
    pub fn new<B: BlockBuilder<C>>(cfg: EpochConfig<B, C>) -> Self {
        let (send_confirmed, recv_confirmed) = smol::channel::unbounded();

        let blockgraph = BlockGraph::new(cfg.genesis.clone());
        let cstate = Arc::new(RwLock::new(ChainState::new(
            cfg.genesis.clone(),
            cfg.forest.clone(),
            blockgraph,
        )));
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
        self.cstate.write().reset_genesis(genesis)
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
    cstate: Arc<RwLock<ChainState<C>>>,
    network: &melnet::NetState,
    voter_key: Ed25519SK,
) -> Result<(), ProtocolError> {
    // Send a summary to a random peer
    let summary = cstate.read().blockgraph.summarize();
    if let Some(rnd_peer) = network.routes().get(0) {
        let diff = melnet::request::<_, Vec<BlockGraphDiff>>(
                *rnd_peer,
                "symphgossip",
                "get_diff",
                summary,
            )
            //.timeout(Duration::from_secs(10))
            //.map_err(|e| log::warn!("gossip request failed with peer {rnd_peer}: {e}"))
            .await?;

        // Integrate diff into block graph
        cstate.write().blockgraph.merge_diff(diff)?;

        // Vote for all lnc tips
        cstate.write().blockgraph.vote_all(voter_key);

        Ok(())
    } else {
        //log::warn!("Failed to get a peer")
        Err(ProtocolError::Custom("Failed to get a peer".into()))
    }
}

/// Communicate summaries to peers and integrate diff responses into the chain state
async fn gossip<C: ContentAddrStore>(
    cstate: Arc<RwLock<ChainState<C>>>,
    network: melnet::NetState,
    send_confirmed: Sender<ConfirmedState<C>>,
    voter_key: Ed25519SK,
) {
    let cstate_inner = cstate.clone();
    network.listen("get_diff", move |breq: Request<crate::blockgraph::Summary>| {
        let cstate_inner = cstate_inner.clone();
        async move {
            let response = cstate_inner.read().blockgraph.partial_summary_diff(&breq.body);
            Ok(response)
        }
    });
    loop {
        // Get a blockgraph update from a random neighbor
        gossip_and_add_diff(cstate.clone(), &network, voter_key)
            .map_err(|e| log::warn!("Error in gossip task: {e}"))
            .await;

        // Drain any new finalized blocks
        let finalized = cstate.write().drain_finalized();
        for block in finalized {
            // TODO obviously confirm should contain a real consensus proof
            // but right now confirm doesn't actually check
            send_confirmed.send(
                block.confirm(BTreeMap::new(), None)
                    .expect("Could not confirm finalized block")).await
                .expect("Failed to send a block on finalized channel");
        }

        smol::Timer::after(Duration::from_secs(1)).await;
    }
}

async fn protocol_loop<B: BlockBuilder<C>, C: ContentAddrStore>(
    cfg: EpochConfig<B, C>,
    cstate: Arc<RwLock<ChainState<C>>>,
    send_confirmed: Sender<ConfirmedState<C>>,
) -> ! {
    let (send_finalized, recv_finalized) = smol::channel::unbounded();

    let cfg = Arc::new(cfg);
    //let height_to_proposer = gen_get_proposer(cfg.genesis.clone()).await;
    let height_to_proposer = gen_get_proposer(cfg.genesis.clone());
    let network = melnet::NetState::new_with_name("symphgossip");
    for addr in &cfg.bootstrap {
        network.add_route(*addr);
    }

    let my_epoch = (cfg.genesis.inner_ref().height + 1.into()).epoch();

    // melnet server
    {
        let cstate_inner = cstate.clone();
        network.listen("get_blocks", move |breq: Request<BlockRequest>| {
            let cstate_inner = cstate_inner.clone();
            async move {
                let response = cstate_inner.read().new_block_responses(breq.body);
                Ok(response)
            }
        });
        let cstate_inner = cstate.clone();
        network.listen("get_txx", move |breq: Request<TransactionRequest>| {
            let cstate_inner = cstate_inner.clone();
            async move {
                let resp = cstate_inner.read().new_transaction_response(breq.body);
                Ok(resp)
            }
        })
    }

    // Spawn gossip loop
    let _gossiper = NS_EXECUTOR.spawn(gossip(
        cstate.clone(),
        network.clone(),
        send_confirmed.clone(),
        cfg.signing_sk
    ));

    //let _gossiper = NS_EXECUTOR.spawn(gossiper_loop(network.clone(), cstate.clone(), cfg.clone()));
    let _confirmer = NS_EXECUTOR.spawn(confirmer_loop(
        my_epoch,
        cfg.signing_sk,
        network.clone(),
        cstate.clone(),
        recv_finalized,
        send_confirmed,
    ));

    // actually run off into the background
    network.add_route(cfg.listen);
    let listener = smol::net::TcpListener::bind(cfg.listen)
        .await
        .expect("could not start to listen");
    let net_inner = network.clone();
    let _server = NS_EXECUTOR.spawn(async move { net_inner.run_server(listener).await });
    loop {
        let vote_loop = async {
            loop {
                cstate.write().vote_all(cfg.signing_sk);
                for block in cstate.write().blockgraph.drain_finalized() {
                    println!("!FINALIZED a block!");
                    let _ = send_finalized.try_send(block);
                }
                let hint_tip = cstate.read().get_lnc_state();
                cfg.builder.hint_next_build(hint_tip);
                smol::Timer::after(Duration::from_secs(1)).await;
            }
        };
        let (height, height_time) = next_height_time(
            cstate.read().get_lnc_state().inner_ref().height,
            cfg.start_time,
            cfg.interval,
        );
        log::debug!("waiting until height_time {:?}", height_time);
        wait_until_sys(height_time).or(vote_loop).await;

        log::debug!("entering height {}", height);

        // let mut cstate = cstate.write();
        if height_to_proposer(height) == cfg.signing_sk.to_public() {
            log::debug!("we are the proposer for height {}", height);
            let mut build_upon = cstate.read().get_lnc_state();
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
            while build_upon.inner_ref().height + BlockHeight(1) < height {
                build_upon = smol::unblock(move || build_upon.next_state().seal(None)).await;
                log::debug!("building empty block for {}", build_upon.inner_ref().height)
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

            let proposed_block = if out_of_bounds {
                build_upon
                    .next_state()
                    .seal(Some(ProposerAction {
                        fee_multiplier_delta: 0,
                        reward_dest: HashVal::default().into(),
                    }))
                    .to_block()
            } else {
                cfg.builder.build_block(build_upon.clone())
            };
            // inject proposal
            if let Err(err) = cstate.write().inject_proposal(
                &proposed_block,
                cfg.signing_sk.to_public(),
                ProposalSig::generate(cfg.signing_sk, &proposed_block.abbreviate()),
                last_nonempty_hash,
            ) {
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
            cstate.write().vote_all(cfg.signing_sk);
        } else {
            log::debug!("we are NOT the proposer for height {}", height);
        }
    }
}

// "gossiper" thread
async fn gossiper_loop<B: BlockBuilder<C>, C: ContentAddrStore>(
    network: melnet::NetState,
    cstate: Arc<RwLock<ChainState<C>>>,
    cfg: Arc<EpochConfig<B, C>>,
) -> ! {
    'mainloop: loop {
        smol::Timer::after(Duration::from_millis(300)).await;
        if let Some(random_peer) = network.routes().get(0) {
            // log::debug!("gossipping with {}", random_pee);
            // create a new block request
            let block_req = cstate.read().new_block_request();
            let response = melnet::request::<_, Vec<AbbrBlockResponse>>(
                *random_peer,
                "symphgossip",
                "get_blocks",
                block_req,
            )
            .timeout(Duration::from_secs(10))
            .await;
            match response {
                None => log::warn!("gossip timed out with {}", random_peer),
                Some(Err(err)) => log::warn!("gossip failed with {}: {:?}", random_peer, err),
                Some(Ok(mut res)) => {
                    // log::debug!("({}) {} responses gotten", random_peer, res.len());
                    res.sort_unstable_by_key(|v| v.abbr_block.header.height);
                    // we now "fill in" everything
                    let mut full_responses = vec![];
                    for abbr_response in res {
                        let mut known = HashSet::new();
                        let mut unknown = Vec::new();
                        // we assemble all the things we don't know
                        for txhash in abbr_response.abbr_block.txhashes.iter().copied() {
                            if let Some(tx) = cfg.builder.get_cached_transaction(txhash) {
                                known.insert(tx);
                            } else {
                                unknown.push(txhash);
                            }
                        }
                        log::trace!(
                            "({}) {} known, {} unknown for {}",
                            random_peer,
                            known.len(),
                            unknown.len(),
                            abbr_response.abbr_block.header.height
                        );
                        // if there are any unknown, send a query to the other side to ask about them
                        if !unknown.is_empty() {
                            log::debug!(
                                "({}) sending query for {} unknowns",
                                random_peer,
                                unknown.len()
                            );
                            let query = TransactionRequest {
                                block_hash: abbr_response.abbr_block.header.hash(),
                                hashes: unknown.clone(),
                            };
                            let response = melnet::request::<_, TransactionResponse>(
                                *random_peer,
                                "symphgossip",
                                "get_txx",
                                query,
                            )
                            .await;
                            match response {
                                Err(err) => {
                                    log::warn!("({}) get_txx failed: {:?}", random_peer, err);
                                    continue 'mainloop;
                                }
                                Ok(response) => {
                                    if response.transactions.len() != unknown.len() {
                                        log::warn!(
                                            "({}) get_txx didn't give us enough",
                                            random_peer
                                        );
                                        continue 'mainloop;
                                    }
                                    for (txhash, transaction) in
                                        unknown.into_iter().zip(response.transactions.into_iter())
                                    {
                                        if transaction.hash_nosigs() != txhash {
                                            log::warn!("({}) get_txx didn't give us something of the right hash", random_peer);
                                            continue 'mainloop;
                                        }
                                        known.insert(transaction);
                                    }
                                }
                            }
                        }
                        // Make the block
                        let block = Block {
                            header: abbr_response.abbr_block.header,
                            transactions: known,
                            proposer_action: abbr_response.abbr_block.proposer_action,
                        };
                        let full_resp = FullBlockResponse {
                            block,
                            metadata: abbr_response.metadata,
                            last_nonempty: abbr_response.last_nonempty,
                        };
                        full_responses.push(full_resp);
                    }
                    let mut cstate = cstate.write();
                    if !full_responses.is_empty() {
                        log::trace!("({}) applying {} blocks", random_peer, full_responses.len());
                    }
                    for full_resp in full_responses {
                        if let Err(err) = cstate.apply_block_response(full_resp) {
                            log::warn!("({}) apply block error: {}", random_peer, err);
                        }
                    }
                }
            }
        }
    }
}

// "gossiper" thread
async fn confirmer_loop<C: ContentAddrStore>(
    my_epoch: u64,
    signing_sk: Ed25519SK,
    network: melnet::NetState,
    cstate: Arc<RwLock<ChainState<C>>>,
    recv_finalized: Receiver<SealedState<C>>,
    send_confirmed: Sender<ConfirmedState<C>>,
) -> Option<()> {
    let known_votes = Arc::new(RwLock::new(BTreeMap::new()));
    network.listen("confirm_block", {
        let known_votes = known_votes.clone();
        move |req: Request<BlockHeight>| {
            let known_votes = known_votes.clone();
            async move {
                let height = req.body;
                let res = known_votes
                    .read()
                    .get(&height)
                    .cloned()
                    .map(|v: UnconfirmedBlock<C>| v.signatures)
                    .unwrap_or_default();
                log::debug!(
                    "responding to confirm request for {} with {} sigs",
                    height,
                    res.len()
                );
                Ok(res)
            }
        }
    });

    let (send_fut, recv_fut) = smol::channel::bounded(128);
    let mut confirmed_generator = FuturesOrdered::<Boxed<Option<ConfirmedState<C>>>>::new();
    let _piper = {
        // let cstate = cstate.clone();
        // let known_votes = known_votes.clone();
        NS_EXECUTOR.spawn(async move {
            loop {
                let start_evt = async {
                    let fut = recv_fut.recv().await.unwrap();
                    Some(fut)
                };
                let end_evt = async {
                    if let Some(res) = confirmed_generator.next().await {
                        if let Some(res) = res {
                            send_confirmed.send(res).await.unwrap();
                        }
                        None
                    } else {
                        smol::future::pending().await
                    }
                };

                if let Some(fut) = start_evt.or(end_evt).await {
                    confirmed_generator.push(fut);
                }
            }
        })
    };

    loop {
        let finalized = recv_finalized.recv().await.ok()?;
        if finalized.inner_ref().height.epoch() > my_epoch {
            log::warn!("skipping out-of-bounds finalized block");
            continue;
        }
        log::info!(
            "[[[ {} FINALIZED (epoch {}) ]]]",
            finalized.inner_ref().height,
            my_epoch
        );
        let my_header = finalized.header();
        let own_signature = signing_sk.sign(&finalized.header().hash());
        let sigs = UnconfirmedBlock {
            state: finalized,
            signatures: [(signing_sk.to_public(), own_signature)]
                .iter()
                .cloned()
                .collect(),
        };
        let my_height = sigs.state.inner_ref().height;
        known_votes.write().insert(my_height, sigs);
        let known_votes = known_votes.clone();
        let cstate = cstate.clone();
        let network = network.clone();

        // This future resolves to either a confirmed block, or nothing. Nothing is when the cstate no longer has this block due to external intervention.
        let confirm_fut = async move {
            while !known_votes
                .read()
                .get(&my_height)
                .unwrap()
                .is_confirmed(cstate.read().stakes())
            {
                if !cstate.read().has_block(my_header.previous) {
                    log::warn!("breaking out of confirmation loop due to external intervention");
                    break;
                }
                if let Some(random_peer) = network.routes().into_iter().next() {
                    // log::debug!(
                    //     "confirming block {} with {}; known votes {:?}",
                    //     my_height,
                    //     random_peer,
                    //     known_votes
                    //         .read()
                    //         .get(&my_height)
                    //         .unwrap()
                    //         .signatures
                    //         .keys()
                    //         .collect::<Vec<_>>()
                    // );
                    let their_sigs = melnet::request::<_, BTreeMap<Ed25519PK, Vec<u8>>>(
                        random_peer,
                        "symphgossip",
                        "confirm_block",
                        my_height,
                    )
                    .await;
                    let mut known_votes = known_votes.write();
                    let sigs = known_votes.get_mut(&my_height).unwrap();
                    match their_sigs {
                        Ok(their_sigs) => {
                            // log::debug!(
                            //     "got {} confirmation sigs from {}",
                            //     their_sigs.len(),
                            //     random_peer
                            // );
                            for (key, signature) in their_sigs {
                                if cstate
                                    .read()
                                    .stakes()
                                    .vote_power(sigs.state.inner_ref().height.epoch(), key)
                                    > 0.0
                                    && key.verify(&sigs.state.header().hash(), &signature)
                                {
                                    sigs.signatures.insert(key, signature);
                                }
                            }
                        }
                        Err(err) => log::warn!(
                            "confirming block {} with {} failed: {:?}",
                            sigs.state.inner_ref().height,
                            random_peer,
                            err
                        ),
                    }
                }
                smol::Timer::after(Duration::from_millis(1000)).await;
            }

            let sigs = known_votes.read().get(&my_height).cloned().unwrap();
            log::info!("[[[ {} CONFIRMED !!! ]]]", &my_height);
            Some(sigs.state.confirm(sigs.signatures, None).unwrap())
        };
        send_fut.send(confirm_fut.boxed()).await.unwrap();
    }
}

#[derive(Derivative, Debug)]
#[derivative(Clone(bound = ""))]
struct UnconfirmedBlock<C: ContentAddrStore> {
    state: SealedState<C>,
    signatures: ConsensusProof,
}

impl<C: ContentAddrStore> UnconfirmedBlock<C> {
    fn is_confirmed(&self, stakes: &StakeMapping<C>) -> bool {
        let mut sum_weights = 0.0;
        for (k, v) in self.signatures.iter() {
            assert!(k.verify(&self.state.header().hash(), v));
            sum_weights += stakes.vote_power(self.state.inner_ref().height.epoch(), *k);
        }
        sum_weights > 0.67
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
        log::debug!("{} staked for epoch {}", total_staked, epoch);
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
                dbg!(seed, sum);
                if seed <= sum {
                    return stake.pubkey;
                }
            }
        }
        unreachable!()
    }
}
