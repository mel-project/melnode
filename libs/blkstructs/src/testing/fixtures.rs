use rstest::*;
use crate::testing::utils::*;
use crate::{Transaction, melscript, CoinID, CoinData, MICRO_CONVERTER, COINTYPE_TMEL};
use tmelcrypt::{Ed25519PK, Ed25519SK};

/// Return a keypair
#[fixture]
pub fn keypair() -> (Ed25519PK, Ed25519SK) {
    tmelcrypt::ed25519_keygen()
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
            conshash: scr.hash(),
            value: MICRO_CONVERTER * 1000,
            cointype: COINTYPE_TMEL.to_owned(),
        },
        sk,
        &scr,
    );
    txx
}