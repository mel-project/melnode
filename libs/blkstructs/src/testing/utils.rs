use std::collections::BinaryHeap;

use crate::{CoinData, CoinID, DENOM_TMEL, melvm, Transaction, TxKind, DENOM_NEWCOIN};
use crate::testing::factory::{TransactionFactory, CoinDataFactory};
use tmelcrypt::{Ed25519PK, Ed25519SK};

pub fn random_valid_txx(
    rng: &mut impl rand::Rng,
    start_coin: CoinID,
    start_coindata: CoinData,
    signer: tmelcrypt::Ed25519SK,
    covenant: &melvm::Covenant,
    fee: u128
) -> Vec<Transaction> {
    random_valid_txx_count(rng, start_coin, start_coindata, signer, covenant, fee, 100)
}

pub fn random_valid_txx_count(
    rng: &mut impl rand::Rng,
    start_coin: CoinID,
    start_coindata: CoinData,
    signer: tmelcrypt::Ed25519SK,
    covenant: &melvm::Covenant,
    fee: u128,
    tx_count: u32
) -> Vec<Transaction> {
    let mut pqueue: BinaryHeap<(u64, CoinID, CoinData)> = BinaryHeap::new();
    pqueue.push((rng.gen(), start_coin, start_coindata));
    let mut toret = Vec::new();
    for _ in 0..tx_count {
        // pop one item from pqueue
        let (_, to_spend, to_spend_data) = pqueue.pop().unwrap();
        assert_eq!(to_spend_data.covhash, covenant.hash());
        let mut new_tx = Transaction {
            kind: TxKind::Normal,
            inputs: vec![to_spend],
            outputs: vec![CoinData {
                covhash: covenant.hash(),
                value: to_spend_data.value - fee,
                denom: DENOM_TMEL.to_owned(),
            }],
            fee,
            scripts: vec![covenant.clone()],
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

pub fn fee_estimate() -> u128 {
    /// Assuming some fee for tx (use higher multiplier to ensure its enough)
    let fee_multiplier = 10000;
    let fee = TransactionFactory::new().build(|_| {}).weight(0).saturating_mul(fee_multiplier);
    fee
}

/// Create a token create transaction from a coin id and the keypair of the cretor
pub fn tx_create_token(signed_keypair: &(Ed25519PK, Ed25519SK), coin_id: &CoinID, unspent_mel_value: u128) -> Transaction {
    let new_coin_tx = TransactionFactory::new().build(|tx| {
        // Create tx outputs
        let tx_fee = fee_estimate();
        let tx_max_token_supply = 1 << 64;
        let tx_coin_params: Vec<(u128, Vec<u8>)> = vec![(unspent_mel_value - tx_fee, DENOM_TMEL.to_vec()), (tx_max_token_supply, DENOM_NEWCOIN.to_vec())];
        let tx_outputs = tx_coin_params
            .iter()
            .map(|(val, denom)| CoinDataFactory::new().build(|cd| {
                cd.value = *val;
                cd.denom = denom.clone();
            }))
            .collect::<Vec<_>>();

        // Create tx covenant hashees
        let tx_scripts = vec![melvm::Covenant::std_ed25519_pk(signed_keypair.0)].to_vec();

        tx.inputs = vec![coin_id.clone()].to_vec();
        tx.outputs = tx_outputs;
        tx.scripts = tx_scripts;
        tx.fee = tx_fee;
    });

    new_coin_tx.sign_ed25519(signed_keypair.1)
}