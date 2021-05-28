use blkstructs::{
    Block, ConfirmedState, ConsensusProof, ProposerAction, SealedState, StakeMapping, Transaction,
    STAKE_EPOCH,
};
use futures_util::stream::FuturesOrdered;
use melnet::Request;
use parking_lot::RwLock;
use smol::{channel::Receiver, future::Boxed};
use smol::{channel::Sender, prelude::*};
use smol_timeout::TimeoutExt;
use std::{
    collections::BTreeMap,
    convert::TryInto,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tmelcrypt::{Ed25519PK, Ed25519SK, HashVal};

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

/// A trait that represents a "mempool".
pub trait BlockBuilder: 'static + Send + Sync {
    /// Given a previous state, build a block that extends it
    fn build_block(&self, tip: SealedState) -> Block;

    /// Sets a "hint" that the next block will extend from a particular state.
    #[allow(unused_variables)]
    fn hint_next_build(&self, tip: SealedState) {}

    /// Gets a cached transaction if available.
    #[allow(unused_variables)]
    fn get_cached_transaction(&self, txhash: HashVal) -> Option<Transaction> {
        None
    }
}

/// Configuration for a running protocol.
pub struct EpochConfig<B: BlockBuilder> {
    pub listen: SocketAddr,
    pub bootstrap: Vec<SocketAddr>,
    pub genesis: SealedState,
    pub forest: novasmt::Forest,
    pub start_time: SystemTime,
    pub interval: Duration,
    pub signing_sk: Ed25519SK,
    pub builder: B,
    pub get_confirmed: Box<dyn Fn(u64) -> Option<ConfirmedState> + Sync + Send + 'static>,
}

/// Represents a running instance of the Symphonia protocol for a particular epoch.
pub struct EpochProtocol {
    _task: smol::Task<()>,
    cstate: Arc<RwLock<ChainState>>,
    recv_confirmed: Receiver<ConfirmedState>,
}

impl EpochProtocol {
    /// Create a new instance of the protocol over melnet.
    pub fn new<B: BlockBuilder>(cfg: EpochConfig<B>) -> Self {
        let (send_confirmed, recv_confirmed) = smol::channel::unbounded();
        let cstate = Arc::new(RwLock::new(ChainState::new(
            cfg.genesis.clone(),
            cfg.forest.clone(),
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
    pub async fn next_confirmed(&self) -> ConfirmedState {
        self.recv_confirmed.recv().await.unwrap()
    }

    /// Forces the given state to be genesis.
    pub fn reset_genesis(&self, genesis: SealedState) {
        self.cstate.write().reset_genesis(genesis)
    }
}

async fn protocol_loop<B: BlockBuilder>(
    cfg: EpochConfig<B>,
    cstate: Arc<RwLock<ChainState>>,
    send_confirmed: Sender<ConfirmedState>,
) -> ! {
    let (send_finalized, recv_finalized) = smol::channel::unbounded();

    let cfg = Arc::new(cfg);
    let height_to_proposer = gen_get_proposer(cfg.genesis.clone());
    let network = melnet::NetState::new_with_name("symphgossip");
    for addr in &cfg.bootstrap {
        network.add_route(*addr);
    }

    let my_epoch = cfg.genesis.inner_ref().height / STAKE_EPOCH;

    // melnet server
    {
        let cstate_inner = cstate.clone();
        network.listen(
            "get_blocks",
            move |breq: Request<BlockRequest, Vec<AbbrBlockResponse>>| {
                let cstate_inner = cstate_inner.clone();
                NS_EXECUTOR
                    .spawn(async move {
                        let response = cstate_inner.read().new_block_responses(breq.body);
                        breq.response.send(Ok(response))
                    })
                    .detach();
            },
        );
        let cstate_inner = cstate.clone();
        network.listen(
            "get_txx",
            move |breq: Request<TransactionRequest, TransactionResponse>| {
                let cstate_inner = cstate_inner.clone();
                NS_EXECUTOR
                    .spawn(async move {
                        let resp = cstate_inner.read().new_transaction_response(breq.body);
                        breq.response.send(Ok(resp))
                    })
                    .detach();
            },
        )
    }
    // melnet client
    let _gossiper = NS_EXECUTOR.spawn(gossiper_loop(network.clone(), cstate.clone(), cfg.clone()));
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
                for block in cstate.write().drain_finalized() {
                    let _ = send_finalized.try_send(block);
                }
                let hint_tip = cstate.read().get_lnc_state();
                cfg.builder.hint_next_build(hint_tip);
                smol::Timer::after(Duration::from_secs(1)).await;
            }
        };
        let (height, height_time) = next_height_time(cfg.start_time, cfg.interval);
        wait_until_sys(height_time).or(vote_loop).await;

        log::debug!("entering height {}", height);

        let mut cstate = cstate.write();
        if height_to_proposer(height) == cfg.signing_sk.to_public() {
            let mut build_upon = cstate.get_lnc_state();
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
            while build_upon.inner_ref().height + 1 < height {
                build_upon = build_upon.next_state().seal(None);
            }

            // am i out of bounds?
            let out_of_bounds = (build_upon.inner_ref().height + 1) / STAKE_EPOCH > my_epoch;
            if out_of_bounds {
                log::warn!(
                    "novasymph running out of bounds: {} is out of epoch {}",
                    build_upon.inner_ref().height + 1,
                    my_epoch
                )
            };

            let proposed_block = if out_of_bounds {
                build_upon
                    .next_state()
                    .seal(Some(ProposerAction {
                        fee_multiplier_delta: 0,
                        reward_dest: HashVal::default(),
                    }))
                    .to_block()
            } else {
                cfg.builder.build_block(build_upon.clone())
            };
            // inject proposal
            if let Err(err) = cstate.inject_proposal(
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
            cstate.vote_all(cfg.signing_sk);
        }
    }
}

// "gossiper" thread
async fn gossiper_loop<B: BlockBuilder>(
    network: melnet::NetState,
    cstate: Arc<RwLock<ChainState>>,
    cfg: Arc<EpochConfig<B>>,
) -> ! {
    'mainloop: loop {
        smol::Timer::after(Duration::from_millis(1000)).await;
        if let Some(random_peer) = network.routes().get(0) {
            // log::debug!("gossipping with {}", random_peer);
            // create a new block request
            let block_req = cstate.read().new_block_request();
            let response = melnet::request::<_, Vec<AbbrBlockResponse>>(
                *random_peer,
                "symphgossip",
                "get_blocks",
                block_req,
            )
            .timeout(Duration::from_secs(5))
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
                        let mut known = im::HashSet::new();
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
async fn confirmer_loop(
    my_epoch: u64,
    signing_sk: Ed25519SK,
    network: melnet::NetState,
    cstate: Arc<RwLock<ChainState>>,
    recv_finalized: Receiver<SealedState>,
    send_confirmed: Sender<ConfirmedState>,
) -> Option<()> {
    let known_votes = Arc::new(RwLock::new(BTreeMap::new()));
    network.listen("confirm_block", {
        let known_votes = known_votes.clone();
        move |req: Request<u64, BTreeMap<Ed25519PK, Vec<u8>>>| {
            let known_votes = known_votes.clone();
            NS_EXECUTOR
                .spawn(async move {
                    let height = req.body;
                    let res = known_votes
                        .read()
                        .get(&height)
                        .cloned()
                        .map(|v: UnconfirmedBlock| v.signatures)
                        .unwrap_or_default();
                    log::debug!(
                        "responding to confirm request for {} with {} sigs",
                        height,
                        res.len()
                    );
                    req.response.send(Ok(res))
                })
                .detach();
        }
    });

    let (send_fut, recv_fut) = smol::channel::bounded(128);
    let mut confirmed_generator = FuturesOrdered::<Boxed<Option<ConfirmedState>>>::new();
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
        if finalized.inner_ref().height / STAKE_EPOCH > my_epoch {
            log::warn!("skipping out-of-bounds finalized block");
            continue;
        }
        log::info!("[[[ {} FINALIZED ]]]", finalized.inner_ref().height);
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
                                    .vote_power(sigs.state.inner_ref().height / STAKE_EPOCH, key)
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

#[derive(Clone, Debug)]
struct UnconfirmedBlock {
    state: SealedState,
    signatures: ConsensusProof,
}

impl UnconfirmedBlock {
    fn is_confirmed(&self, stakes: &StakeMapping) -> bool {
        let mut sum_weights = 0.0;
        for (k, v) in self.signatures.iter() {
            assert!(k.verify(&self.state.header().hash(), v));
            sum_weights += stakes.vote_power(self.state.inner_ref().height / STAKE_EPOCH, *k);
        }
        sum_weights > 0.67
    }
}

async fn wait_until_sys(sys: SystemTime) {
    let now = SystemTime::now();
    if sys < now {
        return;
    } else {
        let interv = sys.duration_since(now).unwrap();
        smol::Timer::after(interv).await;
    }
}

/// waits until the next block height, then returns that height
fn next_height_time(start_time: SystemTime, interval: Duration) -> (u64, SystemTime) {
    let now = SystemTime::now();
    let elapsed_time = now
        .duration_since(start_time)
        .expect("clock randomly jumped, that breaks streamlet");
    let next_height = elapsed_time.as_millis() / interval.as_millis();
    let next_height = next_height as u64;
    let next_time = start_time + interval * (next_height as u32 + 1);
    (next_height, next_time)
}

// a helper function that returns a proposer-calculator for a given epoch, given the SealedState before the epoch.
fn gen_get_proposer(pre_epoch: SealedState) -> impl Fn(u64) -> Ed25519PK {
    let end_height = if pre_epoch.inner_ref().height < STAKE_EPOCH {
        0
    } else if pre_epoch.inner_ref().height / STAKE_EPOCH
        != (pre_epoch.inner_ref().height + 1) / STAKE_EPOCH
    {
        pre_epoch.inner_ref().height
    } else {
        (pre_epoch.inner_ref().height / STAKE_EPOCH * STAKE_EPOCH) - 1
    };
    if end_height > 0 {
        assert!(end_height % STAKE_EPOCH == STAKE_EPOCH - 1)
    }
    // majority beacon of all the blocks in the previous epoch
    let beacon_components = if end_height >= STAKE_EPOCH {
        (end_height - STAKE_EPOCH..=end_height)
            .map(|height| pre_epoch.inner_ref().history.get(&height).0.unwrap().hash())
            .collect::<Vec<_>>()
    } else {
        vec![HashVal::default()]
    };
    let seed = tmelcrypt::majority_beacon(&beacon_components);
    let stakes = pre_epoch.inner_ref().stakes.clone();
    move |height: u64| {
        // we sum the number of µsyms staked
        // TODO: overflow?
        let total_staked = stakes
            .val_iter()
            .filter_map(|v| {
                if v.e_post_end > height / STAKE_EPOCH && v.e_start <= height / STAKE_EPOCH {
                    Some(v.syms_staked)
                } else {
                    None
                }
            })
            .sum::<u128>();
        // "clamp" the subseed
        // we hash the seed with the height
        let mut seed = tmelcrypt::hash_keyed(&height.to_be_bytes(), &seed);
        let seed = loop {
            let numseed = u128::from_be_bytes(
                (&tmelcrypt::hash_keyed(&height.to_be_bytes(), &seed).0[0..16])
                    .try_into()
                    .unwrap(),
            );
            let numseed = numseed >> total_staked.leading_zeros();
            if numseed < total_staked {
                break numseed;
            }
            seed = tmelcrypt::hash_single(&seed);
        };
        // now we go through the stakedocs
        let mut stake_docs = stakes.val_iter().collect::<Vec<_>>();
        stake_docs.sort_by_key(|v| v.pubkey);
        let mut sum = 0;
        for stake in stake_docs {
            if stake.e_post_end > height / STAKE_EPOCH && stake.e_start <= height / STAKE_EPOCH {
                sum += stake.syms_staked;
                if seed <= sum {
                    return stake.pubkey;
                }
            }
        }
        unreachable!()
    }
}
