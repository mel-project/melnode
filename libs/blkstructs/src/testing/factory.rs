// Beaver factories only support serializable structs.
// For structs which don't have serialization support
// build the structure manually or use a custom function with sub-factories where appropriate.

use std::collections::HashMap;

use crate::{
    melvm, Block, CoinData, CoinDataHeight, CoinID, Denom, GenesisConfig, Header, NetID,
    ProposerAction, StakeDoc, Transaction, TxKind,
};

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
        denom -> |_| Denom::Mel,
        additional_data -> |_| vec![],
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
        scripts -> |_| vec![melvm::Covenant::always_true()],
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

// beaver::define! {
//     pub GenesisConfigFactory (GenesisConfig) {
//         network -> |_| NetID::Testnet,
//         init_micromels -> |n| n as u128,
//         init_covhash -> |_| tmelcrypt::HashVal::random(),
//         stakes -> |_| HashMap::new(),
//         init_fee_pool -> |n| n as u128,
//     }
// }

beaver::define! {
    pub ProposerActionFactory (ProposerAction) {
        fee_multiplier_delta -> |n| n as i8,
        reward_dest -> |_| tmelcrypt::HashVal::random(),
    }
}

// beaver::define! {
//     pub HeaderFactory (Header) {
//         network -> |_| NetID::Testnet,
//         previous -> |_| tmelcrypt::HashVal::random(),
//         height -> |n| n as u64,
//         history_hash -> |_| tmelcrypt::HashVal::random(),
//         coins_hash -> |_| tmelcrypt::HashVal::random(),
//         transactions_hash -> |_| tmelcrypt::HashVal::random(),
//         fee_pool -> |n| n as u128,
//         fee_multiplier -> |n| n as u128,
//         dosc_speed ->  |n| n as u128,
//         pools_hash -> |_| tmelcrypt::HashVal::random(),
//         stake_hash -> |_| tmelcrypt::HashVal::random(),
//     }
// }

// beaver::define! {
//     pub BlockFactory (Block) {
//         header -> |n| HeaderFactory::build(n),
//         transactions -> |n| TransactionFactory::build_list(3, n).iter().cloned().collect(),
//         proposer_action -> |_| None,
//     }
// }
