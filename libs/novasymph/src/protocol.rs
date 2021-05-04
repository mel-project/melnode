use blkstructs::{Block, ConsensusProof, SealedState, Transaction, STAKE_EPOCH};
use melnet::Request;
use parking_lot::RwLock;
use smol::{channel::Receiver, stream::StreamExt};
use smol::{channel::Sender, prelude::*};
use std::{
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
    msg::{ProposalSig, VoteSig},
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
    pub forest: autosmt::Forest,
    pub start_time: SystemTime,
    pub interval: Duration,
    pub signing_sk: Ed25519SK,
    pub builder: B,
}

/// Represents a running instance of the Symphonia protocol for a particular epoch.
pub struct EpochProtocol {
    _task: smol::Task<()>,
    recv_finalized: Receiver<SealedState>,
}

impl EpochProtocol {
    /// Create a new instance of the protocol over melnet.
    pub fn new<B: BlockBuilder>(cfg: EpochConfig<B>) -> Self {
        let (send_finalized, recv_finalized) = smol::channel::unbounded();
        Self {
            _task: smolscale::spawn(async move {
                protocol_loop(cfg, send_finalized).await;
            }),
            recv_finalized,
        }
    }

    /// Waits for the next finalized block to come out of the protocol.
    pub async fn next_finalized(&self) -> SealedState {
        self.recv_finalized.recv().await.unwrap()
    }
}

async fn protocol_loop<B: BlockBuilder>(
    cfg: EpochConfig<B>,
    send_finalized: Sender<SealedState>,
) -> ! {
    let cfg = Arc::new(cfg);
    let height_to_proposer = gen_get_proposer(cfg.genesis.clone());
    let cstate = Arc::new(RwLock::new(ChainState::new(
        cfg.genesis.clone(),
        cfg.forest.clone(),
    )));
    let network = melnet::NetState::new_with_name("symphgossip");
    for addr in &cfg.bootstrap {
        network.add_route(*addr);
    }

    // melnet server
    {
        let cstate_inner = cstate.clone();
        network.listen(
            "get_blocks",
            move |breq: Request<BlockRequest, Vec<AbbrBlockResponse>>| {
                let response = cstate_inner.read().new_block_responses(breq.body);
                breq.response.send(Ok(response))
            },
        );
        let cstate_inner = cstate.clone();
        network.listen(
            "get_txx",
            move |breq: Request<TransactionRequest, TransactionResponse>| {
                let resp = cstate_inner.read().new_transaction_response(breq.body);
                breq.response.send(Ok(resp))
            },
        )
    }
    // melnet client
    let _gossiper = smolscale::spawn(gossiper_loop(network.clone(), cstate.clone(), cfg.clone()));

    // actually run off into the background
    let listener = smol::net::TcpListener::bind(cfg.listen)
        .await
        .expect("could not start to listen");
    let net_inner = network.clone();
    let _server = smolscale::spawn(async move { net_inner.run_server(listener).await });

    loop {
        let vote_loop = async {
            let mut interv = smol::Timer::interval(Duration::from_millis(100));
            loop {
                interv.next().await;
                cstate.write().vote_all(cfg.signing_sk);
            }
        };
        let (height, height_time) = next_height_time(cfg.start_time, cfg.interval);
        wait_until_sys(height_time).or(vote_loop).await;

        for block in cstate.write().drain_finalized() {
            let _ = send_finalized.try_send(block);
        }

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
            let build_upon_hash = build_upon.header().hash();
            // fill in a bunch of empty blocks until the height matches
            while build_upon.inner_ref().height + 1 < height {
                build_upon = build_upon.next_state().seal(None);
            }
            let proposed_block = cfg.builder.build_block(build_upon);
            // inject proposal
            cstate
                .inject_proposal(
                    &proposed_block,
                    cfg.signing_sk.to_public(),
                    ProposalSig::generate(cfg.signing_sk, &proposed_block.abbreviate()),
                    build_upon_hash,
                )
                .expect("failed to inject a self-created proposal");
            // vote for it myself
            cstate
                .inject_vote(
                    proposed_block.header.hash(),
                    cfg.signing_sk.to_public(),
                    VoteSig::generate(cfg.signing_sk, &proposed_block.abbreviate()),
                )
                .expect("failed to inject my own vote");
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
        smol::Timer::after(Duration::from_millis(100)).await;
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
            .await;
            match response {
                Err(err) => log::warn!("gossip failed with {}: {:?}", random_peer, err),
                Ok(mut res) => {
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
                        log::debug!(
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
                        };
                        full_responses.push(full_resp);
                    }
                    let mut cstate = cstate.write();
                    if !full_responses.is_empty() {
                        log::debug!("({}) applying {} blocks", random_peer, full_responses.len());
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
