#![feature(test)]
extern crate test;

mod constants;
pub mod melscript;
mod state;
mod transaction;
pub use constants::*;
pub use state::*;
pub use transaction::*;

pub mod testing {
    use super::*;
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
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use test::Bencher;

    #[bench]
    fn batch_insertion(b: &mut Bencher) {
        let _ = env_logger::try_init();
        let db = autosmt::wrap_db(autosmt::TrivialDB::new());
        let (pk, sk) = tmelcrypt::ed25519_keygen();
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
        b.iter(|| {
            let mut genesis = State::test_genesis(&db, MICRO_CONVERTER * 1000, scr.hash());
            genesis.apply_tx_batch(&txx).unwrap();
        })
    }

    #[bench]
    fn single_insertion(b: &mut Bencher) {
        let db = autosmt::wrap_db(autosmt::TrivialDB::new());
        let (pk, sk) = tmelcrypt::ed25519_keygen();
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
        b.iter(|| {
            let mut genesis = State::test_genesis(&db, MICRO_CONVERTER * 1000, scr.hash());
            for tx in txx.iter() {
                genesis.apply_tx(tx).unwrap();
            }
        })
    }

    use rand::prelude::*;

    #[test]
    fn state_simple_order_independence() {
        let db = autosmt::wrap_db(autosmt::TrivialDB::new());
        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let scr = melscript::Script::std_ed25519_pk(pk);
        let genesis = State::test_genesis(&db, MICRO_CONVERTER * 1000, scr.hash());
        let first_block = genesis.finalize();
        let mut trng = rand::thread_rng();
        let mut txx = random_valid_txx(
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
        println!("transactions generated");
        let seq_copy = {
            let mut state = first_block.next_state();
            for tx in txx.iter() {
                state.apply_tx(tx).expect("failed application");
            }
            state.finalize().header().hash()
        };
        let copies: Vec<tmelcrypt::HashVal> = (0..2)
            .map(|_| {
                let mut state = first_block.next_state();
                txx.shuffle(&mut trng);
                state.apply_tx_batch(&txx).expect("failed application");
                state.finalize().header().hash()
            })
            .collect();
        for c in copies {
            assert_eq!(c, seq_copy);
        }
    }

    #[test]
    fn smt_mapping() {
        let tree = autosmt::Tree::new(&autosmt::wrap_db(autosmt::TrivialDB::new()));
        let mut map: state::SmtMapping<u64, u64, autosmt::TrivialDB> = state::SmtMapping::new(tree);
        for i in 0..10 {
            map.insert(i, i);
        }
        assert_eq!(
            hex::encode(map.mapping.root_hash()),
            "c817ba6ba9cadabb754ed5195232be8d22dbd98a1eeca0379921c3cc0b414110"
        );
        for i in 0..10 {
            assert_eq!(Some(i), map.get(&i).0);
        }
        map.delete(&5);
        assert_eq!(None, map.get(&5).0);
        for i in 0..10 {
            map.delete(&i);
        }
        assert_eq!(map.mapping.root_hash(), [0; 32]);
    }
}
