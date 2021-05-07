use std::collections::BinaryHeap;

use crate::{
    testing::factory::{CoinDataFactory, TransactionFactory},
    Denom,
};
// use crate::testing::fixtures::SEND_MEL_AMOUNT;
use crate::{melvm, CoinData, CoinID, Transaction, TxKind};
use tmelcrypt::{Ed25519PK, Ed25519SK, HashVal};

pub fn random_valid_txx(
    rng: &mut impl rand::Rng,
    start_coin: CoinID,
    start_coindata: CoinData,
    signer: tmelcrypt::Ed25519SK,
    covenant: &melvm::Covenant,
    fee: u128,
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
    tx_count: u32,
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
                denom: Denom::Mel,
                additional_data: vec![],
            }],
            fee,
            scripts: vec![covenant.clone()],
            data: vec![],
            sigs: vec![],
        };
        new_tx = new_tx.signed_ed25519(signer);
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
    // Assuming some fee for tx (use higher multiplier to ensure its enough)
    let fee_multiplier = 10000;
    let fee = TransactionFactory::new()
        .build(|_| {})
        .weight()
        .saturating_mul(fee_multiplier);
    fee
}

/// Create a transaction which creates a token
pub fn tx_create_token(
    signer_keypair: &(Ed25519PK, Ed25519SK),
    coin_id: &CoinID,
    mel_balance: u128,
    token_supply: u128,
) -> Transaction {
    let new_coin_tx = TransactionFactory::new().build(|tx| {
        // Create tx outputs
        let tx_fee = fee_estimate();
        // Used to create the value and denom field of outputs
        let tx_coin_params: Vec<(u128, Denom)> = vec![
            (mel_balance - tx_fee, Denom::Mel),
            (token_supply, Denom::NewCoin),
        ];
        let tx_outputs = tx_coin_params
            .iter()
            .map(|(val, denom)| {
                CoinDataFactory::new().build(|cd| {
                    cd.value = *val;
                    cd.denom = denom.clone();
                    cd.covhash = melvm::Covenant::std_ed25519_pk_legacy(signer_keypair.0).hash();
                })
            })
            .collect::<Vec<_>>();

        // Create tx covenant hashees
        let tx_scripts = vec![melvm::Covenant::std_ed25519_pk_legacy(signer_keypair.0)].to_vec();

        tx.inputs = vec![coin_id.clone()].to_vec();
        tx.outputs = tx_outputs;
        tx.scripts = tx_scripts;
        tx.fee = tx_fee;
    });

    new_coin_tx.signed_ed25519(signer_keypair.1)
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
    let fee = fee_estimate();
    let factory = TransactionFactory::new();
    let (pk, _sk) = keypair;
    let cov_hash = melvm::Covenant::std_ed25519_pk_legacy(pk.clone()).hash();

    let tx = factory.build(|tx| {
        tx.kind = TxKind::LiqDeposit;
        tx.inputs = vec![token_create_tx.get_coinid(0), token_create_tx.get_coinid(1)];
        tx.outputs = vec![
            CoinData {
                covhash: cov_hash,
                value: mel_amount - fee_estimate(),
                denom: Denom::Mel,
                additional_data: vec![],
            },
            CoinData {
                covhash: cov_hash,
                value: token_amount,
                denom: Denom::Custom(token_create_tx.hash_nosigs()),
                additional_data: vec![],
            },
        ];
        tx.scripts = vec![melvm::Covenant::std_ed25519_pk_legacy(pk.clone())];
        tx.fee = fee;
    });
    tx.signed_ed25519(keypair.1)
}

// Filter tx outputs by PK
// TODO: convert this to hash map?
pub fn filter_tx_outputs_by_pk(pk: &Ed25519PK, outputs: &Vec<CoinData>) -> Vec<(u8, CoinData)> {
    let cov_hash = melvm::Covenant::std_ed25519_pk_legacy(pk.clone()).hash();
    let outputs: Vec<(u8, CoinData)> = outputs
        .iter()
        .filter(|&cd| cd.clone().covhash == cov_hash)
        .enumerate()
        .map(|e| (e.0 as u8, e.1.clone()))
        .collect();
    outputs
}

pub fn tx_send_mels_to(
    keypair_sender: &(Ed25519PK, Ed25519SK),
    coin_id_sender: CoinID,
    receiver_pk: Ed25519PK,
    total_mel_balance: u128,
    mel_send_amount: u128,
) -> Transaction {
    let fee = fee_estimate();

    let cd_factory = CoinDataFactory::new();
    let cd1 = cd_factory.build(|cd| {
        let covhash = melvm::Covenant::std_ed25519_pk_legacy(receiver_pk).hash();
        cd.covhash = covhash;
        cd.value = total_mel_balance - fee;
        cd.denom = Denom::Mel;
    });
    let cd2 = cd_factory.build(|cd| {
        let covhash = melvm::Covenant::std_ed25519_pk_legacy(receiver_pk).hash();
        cd.covhash = covhash;
        cd.value = mel_send_amount;
        cd.denom = Denom::Mel;
    });

    let tx = TransactionFactory::new().build(|tx| {
        tx.inputs = vec![coin_id_sender];
        tx.outputs = vec![cd1.clone(), cd2.clone()];
        tx.fee = fee;
    });

    tx.signed_ed25519(keypair_sender.1)
}

// pub fn create_mel_buy_tx(
//     keypair_sender: &(Ed25519PK, Ed25519SK),
//     coin_id_sender: CoinID,
//     token_create_tx_hash: HashVal,
//     mel_buy_amount: u128,
//     token_sell_amount: u128,
// ) -> Transaction {
//     let fee = fee_estimate();
//
//     let cd_factory = CoinDataFactory::new();
//     let receiver_pk = keypair_sender.0;
//     let cd1 = cd_factory.build(|cd| {
//         let covhash = melvm::Covenant::std_ed25519_pk(receiver_pk).hash();
//         cd.covhash = covhash;
//         cd.value = token_sell_amount;
//         cd.denom = token_create_tx_hash.to_vec();
//     });
//     let cd2 = cd_factory.build(|cd| {
//         let pk = keypair_sender.clone().0;
//         let covhash = melvm::Covenant::std_ed25519_pk(receiver_pk).hash();
//         cd.covhash = covhash;
//         cd.value = mel_buy_amount;
//         cd.denom = DENOM_TMEL.into();
//     });
//
//     let tx = TransactionFactory::new().build(|tx| {
//         tx.kind = TxKind::Swap;
//         tx.inputs = vec![coin_id_sender];
//         tx.outputs = vec![cd1.clone(), cd2.clone()];
//         tx.fee = fee;
//     });
//
//     tx.sign_ed25519(keypair_sender.1)
// }
//
// pub fn create_mel_sell_tx(
//     keypair_sender: &(Ed25519PK, Ed25519SK),
//     coin_id_sender: CoinID,
//     token_create_tx_hash: HashVal,
//     mel_sell_amount: u128,
//     token_buy_amount: u128,
// ) -> Transaction {
//     let fee = fee_estimate();
//
//     let cd_factory = CoinDataFactory::new();
//     let receiver_pk = keypair_sender.0;
//     let cd1 = cd_factory.build(|cd| {
//         let pk = keypair_sender.clone().0;
//         let covhash = melvm::Covenant::std_ed25519_pk(receiver_pk).hash();
//         cd.covhash = covhash;
//         cd.value = mel_sell_amount;
//         cd.denom = DENOM_TMEL.into();
//     });
//
//     let cd2 = cd_factory.build(|cd| {
//         let covhash = melvm::Covenant::std_ed25519_pk(receiver_pk).hash();
//         cd.covhash = covhash;
//         cd.value = token_buy_amount;
//         cd.denom = token_create_tx_hash.to_vec();
//     });
//
//     let tx = TransactionFactory::new().build(|tx| {
//         tx.kind = TxKind::Swap;
//         tx.inputs = vec![coin_id_sender];
//         tx.outputs = vec![cd1.clone(), cd2.clone()];
//         tx.fee = fee;
//     });
//
//     tx.sign_ed25519(keypair_sender.1)
// }
