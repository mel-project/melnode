// // TODO: Replace benchmarking to use criterion crate
// #[bench]
// fn batch_insertion(b: &mut Bencher) {
//     let _ = env_logger::try_init();
//     let (pk, sk) = tmelcrypt::ed25519_keygen();
//     let scr = melvm::Covenant::std_ed25519_pk(pk);
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
//     let scr = melvm::Covenant::std_ed25519_pk(pk);
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
