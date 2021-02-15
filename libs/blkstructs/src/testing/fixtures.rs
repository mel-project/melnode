use crate::melvm::Covenant;
use crate::testing::utils::*;
use crate::{
    melvm, CoinData, CoinDataHeight, CoinID, StakeDoc, State, Transaction, DENOM_TMEL, MAX_COINVAL,
    MICRO_CONVERTER,
};
use rstest::*;
use std::collections::HashMap;
use tmelcrypt::{Ed25519PK, Ed25519SK};

const GENESIS_MEL_SUPPLY: u128 = 1000;
const GENESIS_NUM_STAKERS: u64 = 10;
const GENESIS_EPOCH_START: u64 = 0;
const GENESIS_EPOCH_POST_END: u64 = 1000;
const GENESIS_STAKER_WEIGHT: u128 = 100;

lazy_static! {
    pub static ref DB: autosmt::DBManager = autosmt::DBManager::load(autosmt::MemDB::default());
    pub static ref GENESIS_COV_SCRIPT_KEYPAIR: (Ed25519PK, Ed25519SK) = tmelcrypt::ed25519_keygen();
    pub static ref GENESIS_STAKEHOLDERS: HashMap<(Ed25519PK, Ed25519SK), u128> = {
        let mut stakeholders = HashMap::new();
        for _ in 0..GENESIS_NUM_STAKERS {
            stakeholders.insert(tmelcrypt::ed25519_keygen(), GENESIS_STAKER_WEIGHT);
        }
        stakeholders
    };
}

#[fixture]
pub fn keypair() -> (Ed25519PK, Ed25519SK) {
    tmelcrypt::ed25519_keygen()
}

#[fixture]
pub fn genesis_cov_script_keypair() -> (Ed25519PK, Ed25519SK) {
    (*GENESIS_COV_SCRIPT_KEYPAIR).clone()
}

#[fixture]
pub fn genesis_cov_script(genesis_cov_script_keypair: (Ed25519PK, Ed25519SK)) -> Covenant {
    melvm::Covenant::std_ed25519_pk(genesis_cov_script_keypair.0).clone()
}

#[fixture]
pub fn genesis_stakeholders() -> HashMap<(Ed25519PK, Ed25519SK), u128> {
    (*GENESIS_STAKEHOLDERS).clone()
}

#[fixture]
pub fn genesis_mel_coin_data(genesis_cov_script: Covenant) -> CoinData {
    let genesis_micro_mel_supply = MICRO_CONVERTER * GENESIS_MEL_SUPPLY;
    assert!(genesis_micro_mel_supply <= MAX_COINVAL);
    CoinData {
        covhash: genesis_cov_script.hash(),
        value: genesis_micro_mel_supply,
        denom: DENOM_TMEL.to_vec(),
    }
}

#[fixture]
pub fn genesis_mel_coin_id() -> CoinID {
    CoinID {
        txhash: tmelcrypt::HashVal([0; 32]),
        index: 0,
    }
}

#[fixture]
pub fn genesis_mel_coin_data_height(genesis_mel_coin_data: CoinData) -> CoinDataHeight {
    CoinDataHeight {
        coin_data: genesis_mel_coin_data,
        height: 0,
    }
}

/// Create a genesis state from mel coin and stakeholders
#[fixture]
pub fn genesis_state(
    genesis_mel_coin_id: CoinID,
    genesis_mel_coin_data_height: CoinDataHeight,
    genesis_stakeholders: HashMap<(Ed25519PK, Ed25519SK), u128>,
) -> State {
    // Init empty state with db reference
    let mut state = State::new_empty((*DB).clone());

    // insert initial mel coin supply
    state
        .coins
        .insert(genesis_mel_coin_id, genesis_mel_coin_data_height);

    // Insert stake holders
    for (i, (&keypair, &syms_staked)) in genesis_stakeholders.iter().enumerate() {
        state.stakes.insert(
            tmelcrypt::hash_single(&(i as u64).to_be_bytes()),
            StakeDoc {
                pubkey: keypair.0,
                e_start: GENESIS_EPOCH_START,
                e_post_end: GENESIS_EPOCH_POST_END,
                syms_staked,
            },
        );
    }

    state
}

/// Return a bundle of transactions for a specific keypair
#[fixture]
pub fn valid_txx(keypair: (Ed25519PK, Ed25519SK)) -> Vec<Transaction> {
    let (pk, sk) = keypair;
    let scr = melvm::Covenant::std_ed25519_pk(pk);
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
        1577000,
    );
    txx
}
