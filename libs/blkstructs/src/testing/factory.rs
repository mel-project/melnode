use tmelcrypt::{Ed25519PK, Ed25519SK};

use crate::{CoinData, CoinID, DENOM_TMEL, CoinDataHeight, Transaction, TxKind, melscript};
use crate::melscript::Script;
use std::collections::BinaryHeap;

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

// TODO: block & state factories
