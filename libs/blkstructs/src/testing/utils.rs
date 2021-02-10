use std::collections::BinaryHeap;

use crate::{CoinData, CoinID, DENOM_TMEL, melscript, Transaction, TxKind};

pub fn random_valid_txx(
    rng: &mut impl rand::Rng,
    start_coin: CoinID,
    start_coindata: CoinData,
    signer: tmelcrypt::Ed25519SK,
    cons: &melscript::Script,
    fee: u128
) -> Vec<Transaction> {
    random_valid_txx_count(rng, start_coin, start_coindata, signer, cons, fee, 100)
}

pub fn random_valid_txx_count(
    rng: &mut impl rand::Rng,
    start_coin: CoinID,
    start_coindata: CoinData,
    signer: tmelcrypt::Ed25519SK,
    cons: &melscript::Script,
    fee: u128,
    tx_count: u32
) -> Vec<Transaction> {
    let mut pqueue: BinaryHeap<(u64, CoinID, CoinData)> = BinaryHeap::new();
    pqueue.push((rng.gen(), start_coin, start_coindata));
    let mut toret = Vec::new();
    for _ in 0..tx_count {
        // pop one item from pqueue
        let (_, to_spend, to_spend_data) = pqueue.pop().unwrap();
        assert_eq!(to_spend_data.covhash, cons.hash());
        let mut new_tx = Transaction {
            kind: TxKind::Normal,
            inputs: vec![to_spend],
            outputs: vec![CoinData {
                covhash: cons.hash(),
                value: to_spend_data.value - fee,
                denom: DENOM_TMEL.to_owned(),
            }],
            fee,
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
