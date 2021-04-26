use crate::services::storage::SharedStorage;

use blkstructs::{
    melvm, AbbrBlock, ProposerAction, SealedState, StakeMapping, Transaction, STAKE_EPOCH,
};
use dashmap::DashMap;
use futures_util::stream::{FuturesOrdered, FuturesUnordered};
use neosymph::{msg::ProposalMsg, StreamletCfg, StreamletEvt, SymphGossip};
use smol::{lock::Semaphore, prelude::*};
use std::{collections::BTreeMap, convert::TryInto, net::SocketAddr, sync::Arc, time::Duration};
use tmelcrypt::{Ed25519PK, Ed25519SK, HashVal};
use tracing::instrument;

/// This encapsulates the staker-specific peer-to-peer.
pub struct StakerProtocol {
    _network_task: smol::Task<()>,
}

impl StakerProtocol {
    /// Creates a new instance of the staker protocol.
    pub fn new(
        addr: SocketAddr,
        bootstrap: Vec<SocketAddr>,
        storage: SharedStorage,
        my_sk: Ed25519SK,
    ) -> anyhow::Result<Self> {
        let _network_task = smolscale::spawn(async move {
            loop {
                let unconfirmed_finalized: Arc<DashMap<u64, HashVal>> = Arc::new(DashMap::new());
                let bootstrap = bootstrap.clone();
                let gossiper = {
                    let storage = storage.clone();
                    let unconfirmed_finalized = unconfirmed_finalized.clone();
                    SymphGossip::new(addr, bootstrap, move |height| {
                        let to_sign = if let Some(val) = storage.read().get_state(height) {
                            val.header().hash()
                        } else {
                            *unconfirmed_finalized.get(&height)?.value()
                        };
                        Some((my_sk.to_public(), my_sk.sign(&to_sign)))
                    })
                    .unwrap()
                };
                // we get the correct starting epoch
                let genesis_epoch = storage.read().highest_height() / STAKE_EPOCH;
                for current_epoch in genesis_epoch.. {
                    log::info!("epoch transitioning into {}!", current_epoch);
                    // we race the staker loop with epoch termination. epoch termination for now is just a sleep loop that waits until the last block in the epoch is confirmed.
                    let staker_fut = staker_loop(
                        gossiper.clone(),
                        storage.clone(),
                        unconfirmed_finalized.clone(),
                        my_sk,
                        current_epoch,
                    );
                    let epoch_termination = async {
                        loop {
                            smol::Timer::after(Duration::from_secs(1)).await;
                            if (storage.read().highest_height() + 1) / STAKE_EPOCH != current_epoch
                            {
                                break Ok(());
                            }
                        }
                    };
                    if let Err(err) = staker_fut.race(epoch_termination).await {
                        log::warn!("staker rebooting: {:?}", err);
                        break;
                    }
                }
            }
        });
        Ok(Self { _network_task })
    }
}

struct WrappedSharedStorage(SharedStorage);

impl neosymph::TxLookup for WrappedSharedStorage {
    fn lookup(&self, hash: HashVal) -> Option<Transaction> {
        self.0.read().mempool().lookup(hash)
    }
}

// a helper function that returns a proposer-calculator for a given epoch, given the SealedState before the epoch.
fn gen_get_proposer(pre_epoch: SealedState, stakes: StakeMapping) -> impl Fn(u64) -> Ed25519PK {
    let end_height = if pre_epoch.inner_ref().height < STAKE_EPOCH {
        0
    } else if pre_epoch.inner_ref().height / STAKE_EPOCH
        != (pre_epoch.inner_ref().height + 1) / STAKE_EPOCH
    {
        pre_epoch.inner_ref().height
    } else {
        (pre_epoch.inner_ref().height / STAKE_EPOCH * STAKE_EPOCH) - 1
    };
    // majority beacon of all the blocks in the previous epoch
    let beacon_components = if end_height >= STAKE_EPOCH {
        (end_height - STAKE_EPOCH..=end_height)
            .map(|height| pre_epoch.inner_ref().history.get(&height).0.unwrap().hash())
            .collect::<Vec<_>>()
    } else {
        vec![HashVal::default()]
    };
    let seed = tmelcrypt::majority_beacon(&beacon_components);
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

#[allow(clippy::clippy::or_fun_call)]
#[instrument(skip(gossiper, storage, my_sk))]
async fn staker_loop(
    gossiper: SymphGossip,
    storage: SharedStorage,
    unconfirmed_finalized: Arc<DashMap<u64, HashVal>>,
    my_sk: Ed25519SK,
    epoch: u64,
) -> anyhow::Result<()> {
    let genesis = storage.read().highest_state();
    let stakes = genesis.inner_ref().stakes.clone();
    let config = StreamletCfg {
        network: gossiper.clone(),
        lookup: WrappedSharedStorage(storage.clone()),
        genesis: genesis.clone(),
        stakes: stakes.clone(),
        epoch,
        start_time: std::time::UNIX_EPOCH + Duration::from_secs(1619280000), // Apr 25 2021
        my_sk,
        interval: Duration::from_secs(30),
        get_proposer: Box::new(gen_get_proposer(genesis.clone(), stakes.clone())),
    };
    let mut streamlet = neosymph::Streamlet::new(config).start();

    let mut last_confirmed = genesis.header().height;

    let my_script = melvm::Covenant::std_ed25519_pk(my_sk.to_public());

    loop {
        let evt = streamlet.next_event().await;
        let last_confirmed_state = storage.read().highest_state();
        streamlet.force_finalize(last_confirmed_state.clone());
        match evt {
            StreamletEvt::SolicitProp(last_state, height, prop_send) => {
                // If we already have a higher state, this means that we were offline and need to reboot symphonia.
                if last_confirmed_state.inner_ref().height > last_state.inner_ref().height {
                    log::warn!("LCS has bigger height than we have! Skip this round to be safe.");
                    continue;
                }
                let provis_state = storage.read().mempool().to_state();
                let out_of_bounds = height / blkstructs::STAKE_EPOCH != epoch;
                let action = if !out_of_bounds {
                    Some(ProposerAction {
                        fee_multiplier_delta: 0,
                        reward_dest: my_script.hash(),
                    })
                } else {
                    log::warn!("proposing a SPECIAL block due to out-of-bounds");
                    Some(neosymph::OOB_PROPOSER_ACTION)
                };
                if height == provis_state.height
                    && Some(last_state.header().hash())
                        == provis_state.history.get(&(height - 1)).0.map(|v| v.hash())
                {
                    let proposal = provis_state.clone().seal(action).to_block().abbreviate();
                    log::info!("responding normally to prop solicit with mempool-based proposal (height={}, hash={:?})", proposal.header.height, proposal.header.hash());
                    let prop_msg = ProposalMsg {
                        proposal,
                        last_nonempty: None,
                    };
                    prop_send.send(prop_msg).unwrap();
                    continue;
                }

                log::info!("bad/missing provisional state. proposing a quasiempty block for height {} because our provis height is {:?}.", height, provis_state.height);

                let mut basis = last_state.clone();
                let mut last_nonempty = None;
                while basis.header().height + 1 < height {
                    log::debug!("filling in empty block for {}", basis.header().height);
                    smol::future::yield_now().await;
                    basis = basis.next_state().seal(None);
                    last_nonempty = Some((last_state.header().height, last_state.header().hash()));
                }
                let next = basis.next_state().seal(action);
                prop_send
                    .send(ProposalMsg {
                        proposal: AbbrBlock {
                            header: next.header(),
                            txhashes: im::OrdSet::new(),
                            proposer_action: action,
                        },
                        last_nonempty,
                    })
                    .unwrap();
            }
            StreamletEvt::LastNotarizedTip(state) => {
                // we set the mempool state to the LNT's successor
                log::info!(
                    "setting mempool LNT to height={}, hash={:?}",
                    state.header().height,
                    state.header().hash()
                );
                storage.write().mempool_mut().rebase(state.next_state());
            }
            StreamletEvt::Finalize(states) => {
                log::info!(
                    "gonna finalize {} states: {:?}",
                    states.len(),
                    states.iter().map(|v| v.header().height).collect::<Vec<_>>()
                );

                // For every state we haven't already confirmed, we finalize it.
                for state in states.iter() {
                    unconfirmed_finalized.insert(state.inner_ref().height, state.header().hash());
                }
                let mut confirmation_tasks = FuturesOrdered::new();
                for state in states {
                    // If this state is beyond our epoch, then we do NOT confirm it.
                    if state.header().height / STAKE_EPOCH > epoch {
                        log::warn!(
                            "block {} is BEYOND OUR EPOCH! This means we CANNOT confirm it!",
                            state.header().height
                        );
                        continue;
                    }
                    if state.header().height > last_confirmed {
                        last_confirmed = state.header().height;
                        confirmation_tasks.push(async {
                            let height = state.header().height;
                            let mut consensus_proof = BTreeMap::new();
                            // until we have full strength
                            while consensus_proof
                                .keys()
                                .map(|v| stakes.vote_power(epoch, *v))
                                .sum::<f64>()
                                < 0.7
                            {
                                match gossiper.solicit_confirmation(height).await {
                                    Ok(Some((some_pk, signature))) => {
                                        if !some_pk.verify(&state.header().hash(), &signature) {
                                            log::warn!(
                                                "invalid confirmation for {} from {:?}",
                                                height,
                                                some_pk
                                            );
                                            continue;
                                        }
                                        // great! the signature was correct, so we stuff into the consensus proof
                                        consensus_proof.insert(some_pk, signature);
                                    }
                                    other => {
                                        log::warn!(
                                            "cannot solicit confirmation for height {}: {:#?}",
                                            height,
                                            other
                                        )
                                    }
                                }
                            }
                            (state, consensus_proof)
                        });
                    }
                }
                while let Some((state, proof)) = confirmation_tasks.next().await {
                    let mut storage = storage.write();
                    let block = state.to_block();
                    if let Err(err) = storage.apply_block(block, proof) {
                        log::warn!(
                            "can't apply finalized block {}: {:?}",
                            state.inner_ref().height,
                            err
                        );
                        // break
                    } else {
                        log::debug!("SUCCESSFULLY COMMITTED HEIGHT {}", state.inner_ref().height);
                    }
                    unconfirmed_finalized.remove(&state.inner_ref().height);
                }
            }
        }
    }
}
