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

/// Create a transaction which creates a token
pub fn tx_create_token(
    signer_keypair: &(Ed25519PK, Ed25519SK),
    coin_id: &CoinID,
    mel_balance: u128,
    token_supply: u128
) -> Transaction {
    let new_coin_tx = TransactionFactory::new().build(|tx| {
        // Create tx outputs
        let tx_fee = fee_estimate();
        // Used to create the value and denom field of outputs
        let tx_coin_params: Vec<(u128, Vec<u8>)> = vec![
            (mel_balance - tx_fee, DENOM_TMEL.to_vec()),
            (token_supply, DENOM_NEWCOIN.to_vec())
        ];
        let tx_outputs = tx_coin_params
            .iter()
            .map(|(val, denom)| CoinDataFactory::new().build(|cd| {
                cd.value = *val;
                cd.denom = denom.clone();
            }))
            .collect::<Vec<_>>();

        // Create tx covenant hashees
        let tx_scripts = vec![melvm::Covenant::std_ed25519_pk(signer_keypair.0)].to_vec();

        tx.inputs = vec![coin_id.clone()].to_vec();
        tx.outputs = tx_outputs;
        tx.scripts = tx_scripts;
        tx.fee = tx_fee;
    });

    new_coin_tx.sign_ed25519(signer_keypair.1)
}

// let deposit_tx = Transaction {
// kind: TxKind::LiqDeposit,
// inputs: vec![newcoin_tx.get_coinid(0), newcoin_tx.get_coinid(1)],

// fee: 2000000,
// scripts: vec![melvm::Covenant::std_ed25519_pk(my_pk)],
// data: vec![],
// sigs: vec![],
// }
// .sign_ed25519(my_sk);

pub fn tx_deposit(
    keypair: &(Ed25519PK, Ed25519SK),
    token_create_tx: Transaction,
    token_amount: u128,
    mel_amount: u128,
) -> Transaction {
    let factory = TransactionFactory::new();
    let (pk, _sk) = keypair;
    let cov_hash = melvm::Covenant::std_ed25519_pk(pk.clone()).hash();

    let tx = factory.build(|tx| {
        tx.kind = TxKind::LiqDeposit;
        tx.inputs = vec![token_create_tx.get_coinid(0), token_create_tx.get_coinid(1)];
        tx.outputs = vec![
            CoinData {
                covhash: cov_hash,
                value: mel_amount - fee_estimate(),
                denom: DENOM_TMEL.into(),
            },
            CoinData {
                covhash: cov_hash,
                value: token_amount,
                denom: token_create_tx.hash_nosigs().to_vec(),
            },
        ];
    });
    tx.sign_ed25519(keypair.1);
    tx
}

// Filter tx outputs by PK
// TODO: convert this to hash map
pub fn filter_tx_outputs_by_pk(pk: &Ed25519PK, outputs: &Vec<CoinData>) -> Vec<(u8, CoinData)> {
    let cov_hash = melvm::Covenant::std_ed25519_pk(pk.clone()).hash();
    let outputs: Vec<(u8, CoinData)> = outputs
        .iter().filter(|&cd| cd.clone().covhash == cov_hash).enumerate().map(|e| (e.0 as u8, e.1.clone())).collect();
    outputs
}