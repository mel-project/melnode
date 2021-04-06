use std::time::SystemTime;

use blkstructs::{melvm, AbbrBlock, ProposerAction, Transaction, STAKE_EPOCH};
use neosymph::{
    msg::ProposalMsg, MockNet, Streamlet, StreamletCfg, StreamletEvt, TxLookup, OOB_PROPOSER_ACTION,
};
use once_cell::sync::Lazy;
use smol::prelude::*;
use tmelcrypt::{Ed25519SK, HashVal};

const COUNT: usize = 10;

/// Bunch of secret keys for testing
static TEST_SKK: Lazy<Vec<Ed25519SK>> =
    Lazy::new(|| (0..COUNT).map(|_| tmelcrypt::ed25519_keygen().1).collect());

fn main() {
    env_logger::init();
    let net = MockNet::new();
    smol::future::block_on(async move {
        for i in 0..COUNT {
            smol::spawn(run_instance(net.clone(), i)).detach();
        }
        smol::future::pending::<()>().await;
    })
}

async fn run_instance(net: MockNet, idx: usize) {
    let config = gen_cfg(net, idx);
    let mut streamlet = Streamlet::new(config);
    let events = streamlet.subscribe();
    let evts_loop = async {
        let mut lnc_tip = None;
        loop {
            let (evt, chain) = events.recv().await.unwrap();
            if idx == 0 {
                eprintln!("{}", chain.graphviz());
            }
            match evt {
                StreamletEvt::LastNotarizedTip(tip) => {
                    lnc_tip = Some(tip);
                }
                StreamletEvt::SolicitProp(ss, height, sender) => {
                    eprintln!(
                        "[{}] soliciting with {:?},{} while current is {:?}",
                        idx,
                        ss.header().hash(),
                        height,
                        lnc_tip.as_ref().map(|lnc_tip| lnc_tip.header().hash())
                    );

                    let action = if height / STAKE_EPOCH == 0 {
                        Some(ProposerAction {
                            fee_multiplier_delta: 0,
                            reward_dest: melvm::Covenant::std_ed25519_pk(TEST_SKK[idx].to_public())
                                .hash(),
                        })
                    } else {
                        Some(OOB_PROPOSER_ACTION)
                    };

                    let mut basis = ss.clone();
                    let mut last_nonempty = None;
                    while basis.header().height + 1 < height {
                        basis = basis.next_state().seal(None);
                        last_nonempty = Some((ss.header().height, ss.header().hash()));
                    }
                    let next = basis.next_state().seal(action);
                    sender
                        .send(ProposalMsg {
                            proposal: AbbrBlock {
                                header: next.header(),
                                txhashes: im::HashSet::new(),
                                proposer_action: action,
                            },
                            last_nonempty,
                        })
                        .unwrap();
                }
                StreamletEvt::Finalize(fin) => {
                    eprintln!(
                        "[{}] ***** FINALIZED ***** up to {}",
                        idx,
                        fin.last().unwrap().header().height
                    );
                }
            }
        }
    };
    evts_loop.race(streamlet.run()).await
}

fn gen_cfg(net: MockNet, idx: usize) -> StreamletCfg<MockNet, TrivialLookup> {
    let genesis_state = blkstructs::State::test_genesis(
        autosmt::Forest::load(autosmt::MemDB::default()),
        10000,
        melvm::Covenant::always_true().hash(),
        TEST_SKK
            .iter()
            .map(|v| v.to_public())
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let stakes = genesis_state.stakes.clone();
    StreamletCfg {
        network: net,
        lookup: TrivialLookup {},
        genesis: genesis_state.seal(None),
        stakes,
        epoch: 0,
        start_time: SystemTime::now(),
        my_sk: TEST_SKK[idx],
        get_proposer: Box::new(|height| TEST_SKK[height as usize % TEST_SKK.len()].to_public()),
    }
}

struct TrivialLookup {}

impl TxLookup for TrivialLookup {
    fn lookup(&self, _hash: HashVal) -> Option<Transaction> {
        unimplemented!()
    }
}
