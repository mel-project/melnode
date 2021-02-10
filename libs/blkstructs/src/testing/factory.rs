// Beaver only supports serializable structs.
// For structs which don't have serialization support
// build the structure manually or
// use a custom function with sub-factories where appropriate

use std::collections::{BinaryHeap, HashMap};

use im::HashSet;

use tmelcrypt::{Ed25519PK, Ed25519SK};

use crate::{Block, CoinData, CoinDataHeight, CoinID, DENOM_TMEL, GenesisConfig, Header, melscript, ProposerAction, StakeDoc, Transaction, TxKind};

beaver::define! {
    pub CoinIDFactory (CoinID) {
        txhash -> |_| tmelcrypt::HashVal::random(),
        index -> |n| n as u8,
    }
}

beaver::define! {
    pub CoinDataFactory (CoinData) {
        covhash -> |_| tmelcrypt::HashVal::random(),
        value -> |n| n as u128,
        denom -> |_| DENOM_TMEL.to_vec(),
    }
}

beaver::define! {
    pub CoinDataHeightFactory (CoinDataHeight) {
        coin_data -> |n| CoinDataFactory::build(n),
        height -> |n| n as u64,
    }
}

beaver::define! {
    pub TransactionFactory (Transaction) {
        kind -> |_| TxKind::Normal,
        inputs -> |n| CoinIDFactory::build_list(3, n),
        outputs -> |n| CoinDataFactory::build_list(3, n),
        fee -> |n| n as u128,
        scripts -> |_| vec![melscript::Script::always_true()],
        data -> |_| vec![],
        sigs -> |_| vec![],
    }
}

beaver::define! {
    pub StakeDocFactory (StakeDoc) {
        pubkey -> |_| tmelcrypt::ed25519_keygen().0,
        e_start -> |n| n as u64,
        e_post_end -> |n| n as u64,
        syms_staked -> |n| n as u128,
    }
}

beaver::define! {
    pub GenesisConfigFactory (GenesisConfig) {
        init_micromels -> |n| n as u128,
        init_covhash -> |_| tmelcrypt::HashVal::random(),
        stakes -> |_| HashMap::new(),
        init_fee_pool -> |n| n as u128,
    }
}

beaver::define! {
    pub ProposerActionFactory (ProposerAction) {
        fee_multiplier_delta -> |n| n as i8,
        reward_dest -> |_| tmelcrypt::HashVal::random(),
    }
}

beaver::define! {
    pub HeaderFactory (Header) {
        previous -> |_| tmelcrypt::HashVal::random(),
        height -> |n| n as u64,
        history_hash -> |_| tmelcrypt::HashVal::random(),
        coins_hash -> |_| tmelcrypt::HashVal::random(),
        transactions_hash -> |_| tmelcrypt::HashVal::random(),
        fee_pool -> |n| n as u128,
        fee_multiplier -> |n| n as u128,
        dosc_speed ->  |n| n as u128,
        pools_hash -> |_| tmelcrypt::HashVal::random(),
        stake_doc_hash -> |_| tmelcrypt::HashVal::random(),
    }
}

beaver::define! {
    pub BlockFactory (Block) {
        header -> |n| HeaderFactory::build(n),
        transactions -> |n| TransactionFactory::build_list(3, n).iter().cloned().collect(),
        proposer_action -> |_| None,
    }
}
//
// // TODO: get ride of this... doesnt' belong here should be a  fixture
// pub fn tx_factory(
//     kind: TxKind,
//     sender_keypair: (Ed25519PK, Ed25519SK),
//     dest_pk: Ed25519PK,
//     coin_id: CoinID,
//     script: melscript::Script,
//     value: u128,
//     fee: u128
// ) -> Transaction {
//     let tx = Transaction {
//         kind,
//         inputs: vec![coin_id],
//         outputs: vec![CoinData {
//             covhash: melscript::Script::std_ed25519_pk(dest_pk).hash(),
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