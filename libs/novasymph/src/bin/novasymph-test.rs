use std::{
    net::SocketAddr,
    time::{Duration, SystemTime},
};

use blkstructs::{
    melvm::Covenant, Block, CoinData, Denom, GenesisConfig, NetID, ProposerAction, SealedState,
    StakeDoc, State,
};
use env_logger::Env;
use novasymph::{BlockBuilder, EpochConfig, EpochProtocol};
use once_cell::sync::Lazy;
use tmelcrypt::{Ed25519PK, Ed25519SK, HashVal};

const COUNT: usize = 10;

/// Bunch of secret keys for testing
static TEST_SKK: Lazy<Vec<Ed25519SK>> =
    Lazy::new(|| (0..COUNT).map(|_| tmelcrypt::ed25519_keygen().1).collect());

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("novasymph")).init();
    let forest = novasmt::Forest::new(novasmt::InMemoryBackend::default());
    let genesis = State::genesis(
        &forest,
        GenesisConfig {
            network: NetID::Testnet,
            init_coindata: CoinData {
                denom: Denom::Mel,
                value: 1 << 64,
                additional_data: vec![],
                covhash: HashVal::default(),
            },
            init_fee_pool: 1 << 64,
            stakes: TEST_SKK
                .iter()
                .map(|v| {
                    (
                        tmelcrypt::hash_single(&v.to_public().0),
                        StakeDoc {
                            pubkey: v.to_public(),
                            e_start: 0,
                            e_post_end: 100000,
                            syms_staked: 1,
                        },
                    )
                })
                .collect(),
        },
    )
    .seal(None);
    smol::future::block_on(async move {
        for i in 0..COUNT {
            smol::spawn(run_staker(i, genesis.clone(), forest.clone())).detach();
        }
        smol::future::pending().await
    })
}

fn idx_to_addr(idx: usize) -> SocketAddr {
    format!("127.0.0.1:{}", idx + 20000).parse().unwrap()
}

async fn run_staker(idx: usize, genesis: SealedState, forest: novasmt::Forest) {
    let protocol = EpochProtocol::new(EpochConfig {
        listen: idx_to_addr(idx),
        bootstrap: (0..COUNT).map(idx_to_addr).collect(),
        genesis,
        forest,
        start_time: SystemTime::now(),
        interval: Duration::from_secs(5),
        signing_sk: TEST_SKK[idx],
        builder: TrivialBlockBuilder {
            pk: TEST_SKK[idx].to_public(),
        },
        get_confirmed: Box::new(|_| None),
    });
    loop {
        let blk = protocol.next_confirmed().await;
        log::warn!("CONFIRMED {:?}", blk.inner().header().height);
        protocol.reset_genesis(blk.inner().clone());
    }
}

struct TrivialBlockBuilder {
    pk: Ed25519PK,
}

impl BlockBuilder for TrivialBlockBuilder {
    fn build_block(&self, tip: SealedState) -> Block {
        tip.next_state()
            .seal(Some(ProposerAction {
                fee_multiplier_delta: 0,
                reward_dest: Covenant::std_ed25519_pk_legacy(self.pk).hash(),
            }))
            .to_block()
    }
}
