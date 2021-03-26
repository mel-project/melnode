use crate::services::storage::SharedStorage;

use blkstructs::{melvm, AbbrBlock, ConsensusProof, ProposerAction, Transaction};
use dashmap::DashMap;
use neosymph::{msg::ProposalMsg, StreamletCfg, StreamletEvt, SymphGossip};
use smol::prelude::*;
use std::{collections::BTreeMap, net::SocketAddr, sync::Arc, time::Duration};
use tmelcrypt::{Ed25519SK, HashVal};
use tracing::instrument;

mod confirm;

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
        let unconfirmed_finalized: Arc<DashMap<u64, HashVal>> = Arc::new(DashMap::new());
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
            })?
        };
        let _network_task = smolscale::spawn(async move {
            staker_loop(gossiper, storage, unconfirmed_finalized, my_sk, 0).await
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

#[allow(clippy::clippy::or_fun_call)]
#[instrument(skip(gossiper, storage, my_sk))]
async fn staker_loop(
    gossiper: SymphGossip,
    storage: SharedStorage,
    unconfirmed_finalized: Arc<DashMap<u64, HashVal>>,
    my_sk: Ed25519SK,
    epoch: u64,
) {
    let genesis = storage.read().highest_state();
    let stakes = genesis.inner_ref().stakes.clone();
    let first_stake = genesis.inner_ref().stakes.val_iter().next().unwrap();
    let config = StreamletCfg {
        network: gossiper.clone(),
        lookup: WrappedSharedStorage(storage.clone()),
        genesis,
        stakes: stakes.clone(),
        epoch,
        start_time: std::time::UNIX_EPOCH + Duration::from_secs(1614578400),
        my_sk,
        get_proposer: Box::new(move |_height| first_stake.pubkey),
    };
    let mut streamlet = neosymph::Streamlet::new(config);
    let events = streamlet.subscribe();

    let mut last_confirmed = 0;

    let my_script = melvm::Covenant::std_ed25519_pk(my_sk.to_public());
    streamlet
        .run()
        .race(async {
            loop {
                let (evt, _) = events.recv().await.unwrap();
                match evt {
                    StreamletEvt::SolicitProp(last_state, height, prop_send) => {
                        let provis_state = storage.read().mempool().to_state();
                        let out_of_bounds = height / blkstructs::STAKE_EPOCH != epoch;

                        let action = if !out_of_bounds {
                            log::info!("bad/missing provisional state. proposing a quasiempty block for height {} because our provis height is {:?}.", height, provis_state.height);
                            Some(ProposerAction {
                            fee_multiplier_delta: 0,
                            reward_dest: my_script.hash(),
                        })} else {
                            log::warn!("proposing a truly empty block due to out-of-bounds");
                            None
                        };
                    if height == provis_state.height
                        && Some(last_state.header().hash())
                            == provis_state.history.get(&(height - 1)).0.map(|v| v.hash())
                    {
                        let proposal = provis_state.clone().seal(action).to_block().abbreviate();
                        log::info!("responding normally to prop solicit with mempool-based proposal (height={}, hash={:?})", proposal.header.height, proposal.header.hash());
                        let prop_msg = ProposalMsg{proposal, last_nonempty: None};
                        prop_send.send(prop_msg).unwrap();
                        continue
                    }
                        let mut basis = last_state.clone();
                        let mut last_nonempty = None;
                        while basis.header().height + 1 < height {
                            log::debug!("filling in empty block for {}", basis.header().height);
                            smol::future::yield_now().await;
                            basis = basis.next_state().seal(None);
                            last_nonempty = Some((last_state.header().height, last_state.header().hash()));
                        }
                        let next = basis.next_state().seal(action);
                        prop_send.send(ProposalMsg {
                            proposal: AbbrBlock {
                                header: next.header(),
                                txhashes: im::HashSet::new(),
                                proposer_action: action,
                            },
                            last_nonempty,
                        })
                        .unwrap();
                    }
                    StreamletEvt::LastNotarizedTip(state) => {
                        // we set the mempool state to the LNT's successor
                        log::info!("setting mempool LNT to height={}, hash={:?}", state.header().height, state.header().hash());
                        storage.write().mempool_mut().rebase(state.next_state());
                    }
                    StreamletEvt::Finalize(states) => {
                        log::info!("gonna finalize {} states: {:?}", states.len(), states.iter().map(|v| v.header().height).collect::<Vec<_>>());

                        // For every state we haven't already confirmed, we finalize it.
                        let mut confirmed_states = vec![];
                        for state in states {
                            if state.header().height > last_confirmed {
                                let height = state.header().height;
                                last_confirmed = height;
                                let mut consensus_proof = BTreeMap::new();
                                unconfirmed_finalized.insert(height, state.header().hash());
                                // until we have full strength
                                while consensus_proof.keys().map(|v| stakes.vote_power(epoch, *v)).sum::<f64>() < 0.7 {
                                    if let Ok(Some((some_pk, signature))) = gossiper.solicit_confirmation(height).await {
                                        log::debug!("got confirmation for {} from {:?}", height, some_pk);
                                        if !some_pk.verify(&state.header().hash(), &signature) {
                                            log::warn!("invalid confirmation for {} from {:?}", height, some_pk);
                                            continue;
                                        }
                                        // great! the signature was correct, so we stuff into the consensus proof
                                        consensus_proof.insert(some_pk, signature);
                                    }
                                }
                                unconfirmed_finalized.remove(&height);
                                log::debug!("CONFIRMED HEIGHT {}", height);
                                confirmed_states.push((state, consensus_proof))
                            }
                        }

                        let mut storage =  storage.write();
                        for (state, proof) in confirmed_states {
                            let block = state.to_block();
                            if let Err(err) = storage.apply_block(block, proof) {
                                log::warn!("can't apply finalized block {}", state.inner_ref().height);
                                // break
                            } else {
                                log::debug!("SUCCESSFULLY COMMITTED HEIGHT {}", state.inner_ref().height);
                            }
                        }
                    }
                }
            }
        })
        .await;
}
