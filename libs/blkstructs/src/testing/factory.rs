use std::collections::{HashMap, BinaryHeap};

use tmelcrypt::{Ed25519PK, Ed25519SK};

use crate::{Block, StakeDoc, CoinData, ProposerAction, CoinDataHeight, CoinID, DENOM_TMEL, GenesisConfig, Header, melscript, Transaction, TxKind};

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
