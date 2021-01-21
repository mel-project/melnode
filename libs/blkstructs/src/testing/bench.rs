use crate::{SmtMapping, melscript, CoinID, CoinData, MICRO_CONVERTER, COINTYPE_TMEL, State};
use crate::testing::utils::random_valid_txx;

// // TODO: Replace benchmarking to use criterion crate
// #[bench]
// fn batch_insertion(b: &mut Bencher) {
//     let _ = env_logger::try_init();
//     let (pk, sk) = tmelcrypt::ed25519_keygen();
//     let scr = melscript::Script::std_ed25519_pk(pk);
//     let mut trng = rand::thread_rng();
//     let txx = random_valid_txx(
//         &mut trng,
//         CoinID {
//             txhash: tmelcrypt::HashVal([0; 32]),
//             index: 0,
//         },
//         CoinData {
//             conshash: scr.hash(),
//             value: MICRO_CONVERTER * 1000,
//             cointype: COINTYPE_TMEL.to_owned(),
//         },
//         sk,
//         &scr,
//     );
//     b.iter(|| {
//         let db = autosmt::DBManager::load(autosmt::MemDB::default());
//         let mut genesis = State::test_genesis(db, MICRO_CONVERTER * 1000, scr.hash(), &[]);
//         genesis.apply_tx_batch(&txx).unwrap();
//     })
// }
//
// #[bench]
// fn single_insertion(b: &mut Bencher) {
//     let (pk, sk) = tmelcrypt::ed25519_keygen();
//     let scr = melscript::Script::std_ed25519_pk(pk);
//     let mut trng = rand::thread_rng();
//     let txx = random_valid_txx(
//         &mut trng,
//         CoinID {
//             txhash: tmelcrypt::HashVal([0; 32]),
//             index: 0,
//         },
//         CoinData {
//             conshash: scr.hash(),
//             value: MICRO_CONVERTER * 1000,
//             cointype: COINTYPE_TMEL.to_owned(),
//         },
//         sk,
//         &scr,
//     );
//     b.iter(|| {
//         let db = autosmt::DBManager::load(autosmt::MemDB::default());
//         let mut genesis = State::test_genesis(db, MICRO_CONVERTER * 1000, scr.hash(), &[]);
//         for tx in txx.iter() {
//             genesis.apply_tx(tx).unwrap();
//         }
//     })
// }
//

// const COUNT: usize = 1;
// /// Bunch of secret keys for testing
// ///
// static TEST_SKK: Lazy<Vec<Ed25519SK>> =
//     Lazy::new(|| (0..COUNT).map(|_| tmelcrypt::ed25519_keygen().1).collect());
//
// #[test]
// fn state_simple_order_independence() {
//     let db = autosmt::DBManager::load(autosmt::MemDB::default());
//     let (pk, sk) = tmelcrypt::ed25519_keygen();
//     let scr = melscript::Script::std_ed25519_pk(pk);
//     let genesis = State::test_genesis(db, MICRO_CONVERTER * 1000, scr.hash(), &[]);
//     let action = Some(ProposerAction {
//         fee_multiplier_delta: 0,
//         reward_dest: melscript::Script::std_ed25519_pk(TEST_SKK[idx].to_public())
//             .hash(),
//     });
//     let first_block = genesis.seal(action); // Pass in proposer action
//     let mut trng = rand::thread_rng();
//     let mut txx = random_valid_txx(
//         &mut trng,
//         CoinID {
//             txhash: tmelcrypt::HashVal([0; 32]),
//             index: 0,
//         },
//         CoinData {
//             conshash: scr.hash(),
//             value: MICRO_CONVERTER * 1000,
//             cointype: COINTYPE_TMEL.to_owned(),
//         },
//         sk,
//         &scr,
//     );
//     println!("transactions generated");
//     let seq_copy = {
//         let mut state = dbg!(first_block.next_state());
//         for tx in txx.iter() {
//             state.apply_tx(tx).expect("failed application");
//         }
//         state.seal(None).header().hash()
//     };
//
//     let action = Some(ProposerAction {
//         fee_multiplier_delta: 0,
//         reward_dest: melscript::Script::std_ed25519_pk(TEST_SKK[idx].to_public())
//             .hash(),
//     });
//
//     let copies: Vec<tmelcrypt::HashVal> = (0..2)
//         .map(|i| {
//             let mut state = dbg!(first_block.next_state());
//             txx.shuffle(&mut trng);
//             state.apply_tx_batch(&txx).expect("failed application");
//             state.seal(action).header().hash()
//         })
//         .collect();
//     for c in copies {
//         assert_eq!(c, seq_copy);
//     }
// }


// TODO: Create an integration/smp_mapping.rs integration test and move this there.
#[test]
fn smt_mapping() {
    let db = autosmt::DBManager::load(autosmt::MemDB::default());
    let tree = db.get_tree(tmelcrypt::HashVal::default());
    let mut map: SmtMapping<u64, u64> = SmtMapping::new(tree.clone());
    for i in 0..10 {
        map.insert(i, i);
    }
    // assert_eq!(
    //     hex::encode(&map.mapping.root_hash()),
    //     "c817ba6ba9cadabb754ed5195232be8d22dbd98a1eeca0379921c3cc0b414110"
    // );
    dbg!(&map);
    let mapbak = map.clone();
    dbg!(&mapbak);
    for i in 0..10 {
        assert_eq!(Some(i), map.get(&i).0);
    }
    map.delete(&5);
    assert_eq!(None, map.get(&5).0);
    for i in 0..10 {
        map.delete(&i);
    }
    dbg!(&mapbak);
    eprintln!("{}", db.debug_graphviz());
    for i in 0..10 {
        assert_eq!(Some(i), mapbak.get(&i).0);
    }
    // assert_eq!(&map.mapping.root_hash(), [0; 32]);
}
