use crate::{CoinID, CoinData, Transaction, melscript, TxKind, COINTYPE_TMEL};
use std::collections::BinaryHeap;

pub fn random_valid_txx(
    rng: &mut impl rand::Rng,
    start_coin: CoinID,
    start_coindata: CoinData,
    signer: tmelcrypt::Ed25519SK,
    cons: &melscript::Script,
) -> Vec<Transaction> {
    let mut pqueue: BinaryHeap<(u64, CoinID, CoinData)> = BinaryHeap::new();
    pqueue.push((rng.gen(), start_coin, start_coindata));
    let mut toret = Vec::new();
    for _ in 0..100 {
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