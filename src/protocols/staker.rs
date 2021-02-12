use crate::services::storage::SharedStorage;

use blkstructs::{melvm, AbbrBlock, ConsensusProof, ProposerAction, Transaction};
use neosymph::{msg::ProposalMsg, StreamletCfg, StreamletEvt, SymphGossip};
use smol::prelude::*;
use std::{net::SocketAddr, time::Duration};
use tmelcrypt::{Ed25519SK, HashVal};
use tracing::instrument;

/// This encapsulates the staker-specific peer-to-peer. At the moment it's very naive, directly using symphonia with blocks, but it can be improved without disrupting the rest of the code.
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
        let gossiper = SymphGossip::new(addr, bootstrap)?;
        let _network_task =
            smolscale::spawn(async move { staker_loop(gossiper, storage, my_sk).await });
        Ok(Self { _network_task })
    }
}

struct WrappedSharedStorage(SharedStorage);

impl neosymph::TxLookup for WrappedSharedStorage {
    fn lookup(&self, hash: HashVal) -> Option<Transaction> {
        self.0.read().lookup(hash)
    }
}

#[allow(clippy::clippy::or_fun_call)]
#[instrument(skip(gossiper, storage, my_sk))]
async fn staker_loop(gossiper: SymphGossip, storage: SharedStorage, my_sk: Ed25519SK) {
    let genesis = storage
        .read()
        .last_block()
        .map(|v| v.inner().clone())
        .unwrap_or(storage.read().genesis());
    let stakes = genesis.inner_ref().stakes.clone();
    let first_stake = genesis.inner_ref().stakes.val_iter().next().unwrap();
    let config = StreamletCfg {
        network: gossiper,
        lookup: WrappedSharedStorage(storage.clone()),
        genesis,
        stakes,
        epoch: 0,
        start_time: std::time::UNIX_EPOCH + Duration::from_secs(1609480800),
        my_sk,
        get_proposer: Box::new(move |_height| first_stake.pubkey),
    };
    let mut streamlet = neosymph::Streamlet::new(config);
    let events = streamlet.subscribe();

    let my_script = melvm::Covenant::std_ed25519_pk(my_sk.to_public());
    let action = Some(ProposerAction {
        fee_multiplier_delta: 0,
        reward_dest: my_script.hash(),
    });
    streamlet
        .run()
        .race(async {
            loop {
                let (evt, _) = events.recv().await.unwrap();
                match evt {
                    StreamletEvt::SolicitProp(last_state, height, prop_send) => {
                        let provis_state = storage.read().provis_state.clone();
                        if let Some(provis_state) = &provis_state {
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
                        }
                        log::info!("bad/missing provisional state. proposing a quasiempty block for height {} because our provis height is {:?}.", height, provis_state.map(|s| s.height));
                        let mut basis = last_state.clone();
                        let mut last_nonempty = None;
                        while basis.header().height + 1 < height {
                            log::debug!("filling in empty block for {}", basis.header().height);
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
                        storage.write().provis_state = Some(state.next_state());
                    }
                    StreamletEvt::Finalize(states) => {
                        log::info!("gonna finalize {} states", states.len());
                        let mut storage =  storage.write();
                        for state in states {
                            let block = state.to_block();
                            if let Err(err) = storage.apply_confirmed_block(block, ConsensusProof::new()) {
                                log::warn!("can't apply finalized block: {:?}", err)
                            }
                        }
                    }
                }
            }
        })
        .await;
}
