use std::collections::BinaryHeap;

use blkstructs::{
    melscript, CoinData, CoinID, State, Transaction, TxKind, COINTYPE_TMEL, MICRO_CONVERTER,
};
use once_cell::sync::Lazy;
use tmelcrypt::{Ed25519PK, Ed25519SK};

fn random_valid_txx(
    rng: &mut impl rand::Rng,
    start_coin: CoinID,
    start_coindata: CoinData,
    signer: tmelcrypt::Ed25519SK,
    cons: &melscript::Script,
) -> Vec<Transaction> {
    let mut pqueue: BinaryHeap<(u64, CoinID, CoinData)> = BinaryHeap::new();
    pqueue.push((rng.gen(), start_coin, start_coindata));
    let mut toret = Vec::new();
    for _ in 0..1000 {
        // pop one item from pqueue
        let (_, to_spend, to_spend_data) = pqueue.pop().unwrap();
        assert_eq!(to_spend_data.conshash, cons.hash());
        let mut new_tx = Transaction {
            kind: TxKind::Normal,
            inputs: vec![to_spend],
            outputs: vec![CoinData {
                conshash: cons.hash(),
                value: to_spend_data.value,
                cointype: COINTYPE_TMEL.to_owned(),
            }],
            fee: 0,
            scripts: vec![cons.clone()],
            data: vec![],
            sigs: vec![],
        };
        new_tx = new_tx.sign_ed25519(signer);
        for (i, out) in new_tx.outputs.iter().enumerate() {
            let cin = CoinID {
                txhash: new_tx.hash_nosigs(),
                index: i as u8,
            };
            pqueue.push((rng.gen(), cin, out.clone()));
        }
        toret.push(new_tx);
    }
    toret
}

static KEYPAIR: Lazy<(Ed25519PK, Ed25519SK)> = Lazy::new(tmelcrypt::ed25519_keygen);

fn main() {
    env_logger::init();
    let db = autosmt::DBManager::load(autosmt::MemDB::default());
    let mut genesis = State::test_genesis(
        db.clone(),
        MICRO_CONVERTER * 1000,
        melscript::Script::std_ed25519_pk(KEYPAIR.0).hash(),
        &[],
    );
    let cov = melscript::Script::std_ed25519_pk(KEYPAIR.0);
    let kmel_cd = CoinData {
        conshash: cov.hash(),
        value: MICRO_CONVERTER * 1000,
        cointype: COINTYPE_TMEL.to_owned(),
    };
    let mut start_coin = CoinID::zero_zero();
    for count in 0..1000 {
        let txx = random_valid_txx(
            &mut rand::thread_rng(),
            start_coin,
            kmel_cd.clone(),
            KEYPAIR.1,
            &cov,
        );
        start_coin = CoinID {
            txhash: txx.last().unwrap().hash_nosigs(),
            index: 0,
        };
        genesis.apply_tx_batch(&txx).unwrap();
        eprintln!("inserted {} batches", count);
        eprintln!("FINALIZING AND CONTINUING!");
        genesis = genesis.finalize().next_state();

        db.sync()
    }
    eprintln!(
        "partial encoding length: {}",
        genesis.partial_encoding().len()
    );
    eprintln!("{:#?}", genesis);
    for coin in genesis.coins.val_iter() {
        eprintln!("{:#?}", coin);
    }
    eprintln!("{}", db.debug_graphviz())
}
