use crate::testing::utils::*;
use crate::{melscript, CoinData, CoinID, Transaction, DENOM_TMEL, MICRO_CONVERTER, State, MAX_COINVAL, StakeDoc};
use rstest::*;
use tmelcrypt::{Ed25519PK, Ed25519SK};
use crate::melscript::Script;
use autosmt::DBManager;
use std::collections::HashMap;

const GENESIS_MELS_SUPPLY: u64 = 1000;
const GENESIS_NUM_STAKERS: u64 = 10;
const GENESIS_EPOCH_START: u64 = 0;
const GENESIS_EPOCH_POST_END: u64 = 1000;
const GENESIS_STAKER_WEIGHT: u64 = 100;

lazy_static! {
    pub static ref DB: autosmt::DBManager = autosmt::DBManager::load(autosmt::MemDB::default());
    pub static ref COV_SCRIPT_KEYPAIR: (Ed25519PK, Ed25519SK) = tmelcrypt::ed25519_keygen();
}

#[fixture]
pub fn db() -> DBManager {
    (*DB).clone()
}

#[fixture]
pub fn cov_script_keypair() -> (Ed25519PK, Ed25519SK) {
    (*COV_SCRIPT_KEYPAIR).clone()
}

#[fixture]
pub fn cov_script(cov_script_keypair: (Ed25519PK, Ed25519SK)) -> melscript::Script {
    melscript::Script::std_ed25519_pk(cov_script_keypair.0)
}

#[fixture]
pub fn genesis_stakeholders() -> HashMap<(Ed25519PK, Ed25519SK), u64> {
    let mut stakeholders = HashMap::new();
    for i in 0..GENESIS_NUM_STAKERS {
        stakeholders.insert(tmelcrypt::ed25519_keygen(), GENESIS_STAKER_WEIGHT);
    }
    stakeholders
}

/// Return a bundle of transactions for a specific keypair
#[fixture]
pub fn valid_txx(keypair: (Ed25519PK, Ed25519SK)) -> Vec<Transaction> {
    let (pk, sk) = keypair;
    let scr = melscript::Script::std_ed25519_pk(pk);
    let mut trng = rand::thread_rng();
    let txx = random_valid_txx(
        &mut trng,
        CoinID {
            txhash: tmelcrypt::HashVal([0; 32]),
            index: 0,
        },
        CoinData {
            covhash: scr.hash(),
            value: MICRO_CONVERTER * 1000,
            denom: DENOM_TMEL.to_owned(),
        },
        sk,
        &scr,
    );
    txx
}

/// Return a genesis state with no stakeholders
#[fixture]
pub fn genesis_state(db: DBManager, cov_script: Script, genesis_stakeholders: HashMap<(Ed25519PK, Ed25519SK), u64>) -> State {
    assert!(start_micro_mels <= MAX_COINVAL);
    let mut state = State::new_empty(db);

    // insert coin out of nowhere
    let init_coin = txn::CoinData {
        covhash: cov_script,
        value: MICRO_CONVERTER * GENESIS_MELS_SUPPLY,
        denom: DENOM_TMEL.to_vec(),
    };
    state.coins.insert(
        txn::CoinID {
            txhash: tmelcrypt::HashVal([0; 32]),
            index: 0,
        },
        txn::CoinDataHeight {
            coin_data: init_coin,
            height: 0,
        },
    );

    // Insert stake holders
    for (i, (keypair, syms_staked)) in genesis_stakeholders.iter().enumerate() {
        state.stakes.insert(
            tmelcrypt::hash_single(&(i as u64).to_be_bytes()),
            StakeDoc {
                pubkey: (*keypair).0,
                e_start: GENESIS_EPOCH_START,
                e_post_end: GENESIS_EPOCH_POST_END,
                syms_staked: *syms_staked,
            },
        );
    }

    state
}