use tmelcrypt::{Ed25519PK, Ed25519SK};

use crate::{CoinData, CoinID, DENOM_TMEL, Transaction, TxKind, melscript};
use crate::melscript::Script;
use std::collections::BinaryHeap;

pub mod factory {
    use crate::{CoinID, CoinData, CoinDataHeight, Transaction, TxKind, DENOM_TMEL, melscript};

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
}

//
// struct TransactionFactory {
//     kind: TxKind,
//     sender_keypair: (Ed25519PK, Ed25519SK),
//     dest_pk: Ed25519PK,
//     coin_id: CoinID,
//     script: Script,
//     value: u128,
//     fee: u128
// }
//
// factori!(Transaction, {
//     default {
//         kind = TxKind::Normal,
//     }
//     builder {
//         let tx = Transaction {
//             kind,
//             inputs: vec![coin_id],
//             outputs: vec![CoinData {
//                 covhash: Script::std_ed25519_pk(dest_pk).hash(),
//             value: value - fee,
//             denom: DENOM_TMEL.to_owned(),
//         }],
//         fee,
//         scripts: vec![script],
//         data: vec![],
//         sigs: vec![]
//     };
//
//     // Sign transaction and return tx
//     tx.sign_ed25519(sender_keypair.1)
//     }
//     mixin deposit {
//         kind = TxKind::LiqDeposit,
//     }
//     mixin swap {
//         kind = TxKind::Swap,
//     }
//     mixin withdraw {
//         kind = TxKind::LiqWithdraw,
//     }
// });
//
// struct TransactionsFactory {
//     num_tx: u64,
// }
//
// factori!(Vec<Transaction>, {
//     default {
//         num_tx: 100
//     }
// });
//
//     // Transaction {
//     //     kind,
//     //     inputs: vec![coin_id],
//     //     outputs: vec![CoinData {
//     //         covhash: Script::std_ed25519_pk(dest_pk).hash(),
//     //         value: value - fee,
//     //         denom: DENOM_TMEL.to_owned(),
//     //     }],
//     //     fee,
//     //     scripts: vec![script],
//     //     data: vec![],
//     //     sigs: vec![]
//     // }
// pub fn tx_factory(
//     kind: TxKind,
//     sender_keypair: (Ed25519PK, Ed25519SK),
//     dest_pk: Ed25519PK,
//     coin_id: CoinID,
//     script: Script,
//     value: u128,
//     fee: u128
// ) -> Transaction {
//     let tx = Transaction {
//         kind,
//         inputs: vec![coin_id],
//         outputs: vec![CoinData {
//             covhash: Script::std_ed25519_pk(dest_pk).hash(),
//             value: value - fee,
//             denom: DENOM_TMEL.to_owned(),
//         }],
//         fee,
//         scripts: vec![script],
//         data: vec![],
//         sigs: vec![]
//     };
//
//     // Sign transaction and return tx
//     tx.sign_ed25519(sender_keypair.1)
// }
//
// pub fn txx_factory(
//     rng: &mut impl rand::Rng,
//     start_coin: CoinID,
//     start_coindata: CoinData,
//     signer: tmelcrypt::Ed25519SK,
//     cons: &melscript::Script,
//     fee: u128,
//     tx_count: u32
// ) -> Vec<Transaction> {
//     let mut pqueue: BinaryHeap<(u64, CoinID, CoinData)> = BinaryHeap::new();
//     pqueue.push((rng.gen(), start_coin, start_coindata));
//     let mut toret = Vec::new();
//     for _ in 0..tx_count {
//         // pop one item from pqueue
//         let (_, to_spend, to_spend_data) = pqueue.pop().unwrap();
//         assert_eq!(to_spend_data.covhash, cons.hash());
//         let mut new_tx = tx_factory(
//
//         )
//         let mut new_tx = Transaction {
//             kind: TxKind::Normal,
//             inputs: vec![to_spend],
//             outputs: vec![CoinData {
//                 covhash: cons.hash(),
//                 value: to_spend_data.value - fee,
//                 denom: DENOM_TMEL.to_owned(),
//             }],
//             fee,
//             scripts: vec![cons.clone()],
//             data: vec![],
//             sigs: vec![],
//         };
//         new_tx = new_tx.sign_ed25519(signer);
//         for (i, out) in new_tx.outputs.iter().enumerate() {
//             let cin = CoinID {
//                 txhash: new_tx.hash_nosigs(),
//                 index: i as u8,
//             };
//             pqueue.push((rng.gen(), cin, out.clone()));
//         }
//         toret.push(new_tx);
//     }
//     toret
// }
