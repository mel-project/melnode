use std::collections::HashMap;

use rstest::*;

use tmelcrypt::{Ed25519PK, Ed25519SK};

use crate::{
    CoinData, CoinDataHeight, CoinID, DENOM_TMEL, MAX_COINVAL, melscript, MICRO_CONVERTER, StakeDoc,
    State, Transaction, GenesisConfig
};
use crate::melscript::Script;
use crate::testing::factory::{CoinDataFactory, CoinDataHeightFactory, GenesisConfigFactory, CoinIDFactory};
use crate::testing::utils::*;

const GENESIS_MEL_SUPPLY: u128 = 1000000;
const GENESIS_NUM_STAKERS: u64 = 10;
const GENESIS_EPOCH_START: u64 = 0;
const GENESIS_EPOCH_POST_END: u64 = 1000;
const GENESIS_STAKER_WEIGHT: u128 = 100;
const GENESIS_INIT_FEE_POOL: u128 = 1000;

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
pub fn genesis_cov_script(genesis_cov_script_keypair: (Ed25519PK, Ed25519SK)) -> Script {
    melscript::Script::std_ed25519_pk(genesis_cov_script_keypair.0).clone()
}

#[fixture]
pub fn genesis_stakeholders() -> HashMap<(Ed25519PK, Ed25519SK), u128> {
    (*GENESIS_STAKEHOLDERS).clone()
}

#[fixture]
pub fn genesis_mel_coin_data(genesis_cov_script: Script) -> CoinData {
    let genesis_micro_mel_supply = MICRO_CONVERTER * GENESIS_MEL_SUPPLY;
    assert!(genesis_micro_mel_supply <= MAX_COINVAL);

    let coin_data_factory = CoinDataFactory::new();
    coin_data_factory.build(|coin_data| {
        coin_data.covhash = genesis_cov_script.hash();
        coin_data.value = genesis_micro_mel_supply;
    })
}

#[fixture]
pub fn genesis_mel_coin_id() -> CoinID {
    let factory = CoinIDFactory::new();

    factory.build(|coin_id| {
        coin_id.txhash = tmelcrypt::HashVal([0; 32]);
        coin_id.index = 0;
    })
}

#[fixture]
pub fn genesis_mel_coin_data_height(genesis_mel_coin_data: CoinData) -> CoinDataHeight {
    let coin_data_height_factory = CoinDataHeightFactory::new();

    coin_data_height_factory.build(|coin_data_height| {
        coin_data_height.coin_data = genesis_mel_coin_data.clone();
        coin_data_height.height = 0;
    })
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
        1577000,
    );
    txx
}

//
// pub fn make_tx() {
//     let coin_id_factory = CoinIDFactory::new();
//
//     coin_id_factory.bu
// }
//
// pub fn tx_factory(
//     kind: TxKind,
//     sender_keypair: (Ed25519PK, Ed25519SK),
//     dest_pk: Ed25519PK,
//     coin_id: CoinID,
//     script: Script,
//     value: u128,
//     fee: u128
// ) -> Transaction {
//     let tx = Transaction {
//         kind,
//         inputs: vec![coin_id],
//         outputs: vec![CoinData {
//             covhash: Script::std_ed25519_pk(dest_pk).hash(),
//             value: value - fee,
//             denom: DENOM_TMEL.to_owned(),
//         }],
//         fee,
//         scripts: vec![script],
//         data: vec![],
//         sigs: vec![]
//     };
//
//     // Sign transaction and return tx
//     tx.sign_ed25519(sender_keypair.1)
// }
//
// pub fn txx_factory(
//     rng: &mut impl rand::Rng,
//     start_coin: CoinID,
//     start_coindata: CoinData,
//     signer: tmelcrypt::Ed25519SK,
//     cons: &melscript::Script,
//     fee: u128,
//     tx_count: u32
// ) -> Vec<Transaction> {
//     let mut pqueue: BinaryHeap<(u64, CoinID, CoinData)> = BinaryHeap::new();
//     pqueue.push((rng.gen(), start_coin, start_coindata));
//     let mut toret = Vec::new();
//     for _ in 0..tx_count {
//         // pop one item from pqueue
//         let (_, to_spend, to_spend_data) = pqueue.pop().unwrap();
//         assert_eq!(to_spend_data.covhash, cons.hash());
//         // let mut new_tx = tx_factory(
//         //
//         // )
//         let mut new_tx = Transaction {
//             kind: TxKind::Normal,
//             inputs: vec![to_spend],
//             outputs: vec![CoinData {
//                 covhash: cons.hash(),
//                 value: to_spend_data.value - fee,
//                 denom: DENOM_TMEL.to_owned(),
//             }],
//             fee,
//             scripts: vec![cons.clone()],
//             data: vec![],
//             sigs: vec![],
//         };
//         new_tx = new_tx.sign_ed25519(signer);
//         for (i, out) in new_tx.outputs.iter().enumerate() {
//             let cin = CoinID {
//                 txhash: new_tx.hash_nosigs(),
//                 index: i as u8,
//             };
//             pqueue.push((rng.gen(), cin, out.clone()));
//         }
//         toret.push(new_tx);
//     }
//     toret
// }
