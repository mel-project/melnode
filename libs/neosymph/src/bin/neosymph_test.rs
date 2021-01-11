use std::{collections::BTreeSet, time::SystemTime};

use blkstructs::{melscript, Transaction};
use neosymph::{
    msg::{AbbrBlock, ProposalMsg},
    MockNet, Streamlet, StreamletCfg, StreamletEvt, TxLookup,
};
use once_cell::sync::Lazy;
use smol::prelude::*;
use tmelcrypt::{Ed25519SK, HashVal};

const COUNT: usize = 16;

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
            eprintln!("[{}] event: {:?}", idx, evt);
            if idx == 0 {
                eprintln!("{}", chain.graphviz());
            }
            match evt {
                StreamletEvt::LastNotarizedTip(tip) => {
                    lnc_tip = Some(tip);
                }
                StreamletEvt::SolicitProp(ss, height, sender) => {
                    eprintln!(
                        "soliciting with {:?},{} while current is {:?}",
                        ss.header().hash(),
                        height,
                        lnc_tip.as_ref().map(|lnc_tip| lnc_tip.header().hash())
                    );
                    let mut next = ss.next_state().seal(None);
                    let mut last_nonempty = None;
                    while next.header().height < height {
                        next = next.next_state().seal(None);
                        last_nonempty = Some((ss.header().height, ss.header().hash()));
                    }
                    sender
                        .send(ProposalMsg {
                            proposal: AbbrBlock {
                                header: next.header(),
                                txhashes: BTreeSet::new(),
                            },
                            last_nonempty,
                        })
                        .unwrap();
                }
            }
        }
    };
    evts_loop.race(streamlet.run()).await
}

fn gen_cfg(net: MockNet, idx: usize) -> StreamletCfg<MockNet, TrivialLookup> {
    let genesis_state = blkstructs::State::test_genesis(
        autosmt::DBManager::load(autosmt::MemDB::default()),
        10000,
        melscript::Script::always_true().hash(),
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
    fn lookup(&self, hash: HashVal) -> Option<Transaction> {
        unimplemented!()
    }
}
