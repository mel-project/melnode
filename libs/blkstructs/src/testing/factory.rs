use std::collections::{HashMap, BinaryHeap};

use tmelcrypt::{Ed25519PK, Ed25519SK};

use crate::{Block, StakeDoc, CoinData, CoinDataHeight, CoinID, DENOM_TMEL, GenesisConfig, Header, melscript, SealedState, State, Transaction, TxKind};

// Beaver only supports serializable structs.
// For structs which don't have serialization support
// build the structure manually or
// use a custom function with sub-factories where appropriate

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


// pub struct ProposerAction {
//     /// Change in fee. This is scaled to the proper size.
//     pub fee_multiplier_delta: i8,
//     /// Where to sweep fees.
//     pub reward_dest: HashVal,
// }

// #[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
// /// A block header.
// pub struct Header {
//     pub previous: HashVal,
//     pub height: u64,
//     pub history_hash: HashVal,
//     pub coins_hash: HashVal,
//     pub transactions_hash: HashVal,
//     pub fee_pool: u128,
//     pub fee_multiplier: u128,
//     pub dosc_speed: u128,
//     pub pools_hash: HashVal,
//     pub stake_doc_hash: HashVal,
// }

// pub struct Block {
//     pub header: Header,
//     pub transactions: im::HashSet<Transaction>,
//     pub proposer_action: Option<ProposerAction>,
// }
//
// /// An abbreviated block
// #[derive(Serialize, Deserialize, Clone, Debug)]
// pub struct AbbrBlock {
//     pub header: Header,
//     pub txhashes: im::HashSet<HashVal>,
//     pub proposer_action: Option<ProposerAction>,
// }
