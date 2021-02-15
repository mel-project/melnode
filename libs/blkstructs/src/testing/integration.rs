use rand::prelude::SliceRandom;

use rstest::*;
use crate::{Block, CoinData, CoinDataHeight, CoinID, DENOM_TMEL, melvm, MICRO_CONVERTER, SmtMapping, State, Transaction, TxKind, DENOM_NEWCOIN};
use crate::testing::fixtures::{genesis_state, tx_send_mel_from_seed_coin};
use crate::testing::factory::*;
use crate::testing::utils::{random_valid_txx, fee_estimate, tx_create_token};
use tmelcrypt::{Ed25519PK, Ed25519SK, HashVal};
use crate::melvm::Covenant;


fn test_melswap_v2_simple(
    genesis_state: State,
    tx_send_mel_from_seed_coin: ((Ed25519PK, Ed25519SK), Transaction)
) {
    // Initialize genesis
    let sealed_state = genesis_state.seal(None);

    // create tx that funds liquidity provider account with mel
    let (liquidity_keypair, tx_liquidity_mel) = tx_send_mel_from_seed_coin;

    // create tx s.t. liquidity provider creates a new token
    let coin_id = tx_liquidity_mel.inputs.first().unwrap();
    let tx_liquidity_token = tx_create_token(liquidity_keypair, coin_id);

    // Get next state, add txx and seal
    let mut first_block = sealed_state.next_state();
    let txx = vec![tx_liquidity_mel, tx_liquidity_token];
    assert!(first_block.apply_tx_batch(txx.as_slice()).is_ok());
    let sealed_state = first_block.seal(None);

    // deposit mel/token keypair into pool
    // Create liquidity deposit tx from coin data
    let fee = fee_estimate();
    let factory = TransactionFactory::new();
    let deposit_tx = factory.build(|dep_tx| {
        dep_tx.kind = TxKind::LiqDeposit;
        dep_tx.fee = fee;
        dep_tx.inputs = vec![];
    });

    // add tx to block and seal first block to add liquidity
    let mut second_block = sealed_state.next_state();
    assert!(second_block.apply_tx(&deposit_tx).is_ok());
    let sealed_state = second_block.seal(None);

    // fund mel buyer account
    let (liquidity_keypair, tx_liquidity_mel) = tx_send_mel_from(tx_liquidity_token.inputs.first());

    // fund mel seller account
    let (liquidity_keypair, tx_liquidity_mel) = tx_send_mel_from(tx_liquidity_token.inputs.first());

    // random swaps and check invariants
    for (..) {
        // check liq_constant
    }

    // withdraw mel/token pair from pool

    // random swaps and check invariants

    // deposit mel/token pair to pool

    // random swaps and check invariants

    // ...
}

#[rstest]
fn test_state_apply_single_deposit_valid_liquidity(
    genesis_state: State,
    tx_send_mel_from_seed_coin: ((Ed25519PK, Ed25519SK), Transaction)
) {
    // Initialize genesis
    let sealed_state = genesis_state.seal(None);

    // Send a tx to keypair from seed coin created in genesis
    let (keypair, tx) = tx_send_mel_from_seed_coin;

    // Get next state and add tx
    let mut next_state = sealed_state.next_state();
    assert!(next_state.apply_tx(&tx).is_ok());

    // Get receiver coin id
    let coin_id = CoinIDFactory::new().build(|cid| {
        cid.txhash = tx.hash_nosigs();
        cid.index = 0;
    });

    // Get coin data
    let (cdh, _) = next_state.coins.get(&coin_id);
    let coin_data = cdh.unwrap().coin_data;

    let coin_id_inputs = CoinIDFactory::new().build_list(2,|_| {});

    // Create liquidity deposit tx from coin data
    let fee = fee_estimate();
    let factory = TransactionFactory::new();
    let deposit_tx = factory.build(|dep_tx| {
        dep_tx.kind = TxKind::LiqDeposit;
        dep_tx.fee = fee;
        dep_tx.inputs = vec![];
    });

    // check total liquidity / sum of all deposits is correct

    // let signed_deposit_tx = deposit_tx.sign_ed25519(keypair.1);
    //
    // let mut next_state = sealed_state.next_state();
    //
    // // check total liquidity / sum of all deposits is correct
    // assert!(next_state.apply_tx(&signed_deposit_tx).is_ok());
    //
    // let mut next_sealed_state = next_state.seal(None);

}
//
// #[test]
// fn state_simple_order_independence() {
//     let db = autosmt::DBManager::load(autosmt::MemDB::default());
//     let (pk, sk) = tmelcrypt::ed25519_keygen();
//     let scr = melvm::Covenant::std_ed25519_pk(pk);
//     let mut genesis = State::test_genesis(db, MICRO_CONVERTER * 1000, scr.hash(), &[]);
//     genesis.fee_multiplier = 0;
//     let first_block = genesis.seal(None);
//     let mut trng = rand::thread_rng();
//     let mut txx = random_valid_txx(
//         &mut trng,
//         CoinID {
//             txhash: tmelcrypt::HashVal([0; 32]),
//             index: 0,
//         },
//         CoinData {
//             covhash: scr.hash(),
//             value: MICRO_CONVERTER * 1000,
//             denom: DENOM_TMEL.to_owned(),
//         },
//         sk,
//         &scr,
//         1577000
//     );
//     println!("transactions generated");
//     let seq_copy = {
//         let mut state = first_block.next_state();
//         for tx in txx.iter() {
//             state.apply_tx(tx).expect("failed application");
//         }
//         dbg!(state.seal(None).header()).hash()
//     };
//     let copies: Vec<tmelcrypt::HashVal> = (0..8)
//         .map(|_i| {
//             let mut state = first_block.next_state();
//             txx.shuffle(&mut trng);
//             state.apply_tx_batch(&txx).expect("failed application");
//             state.seal(None).header().hash()
//         })
//         .collect();
//     for c in copies {
//         assert_eq!(c, seq_copy);
//     }
// }
//
// // TODO: Create an integration/smp_mapping.rs integration test and move this there.
// #[test]
// fn smt_mapping() {
//     let db = autosmt::DBManager::load(autosmt::MemDB::default());
//     let tree = db.get_tree(tmelcrypt::HashVal::default());
//     let mut map: SmtMapping<u64, u64> = SmtMapping::new(tree.clone());
//     for i in 0..10 {
//         map.insert(i, i);
//     }
//     // assert_eq!(
//     //     hex::encode(&map.mapping.root_hash()),
//     //     "c817ba6ba9cadabb754ed5195232be8d22dbd98a1eeca0379921c3cc0b414110"
//     // );
//     dbg!(&map);
//     let mapbak = map.clone();
//     dbg!(&mapbak);
//     for i in 0..10 {
//         assert_eq!(Some(i), map.get(&i).0);
//     }
//     map.delete(&5);
//     assert_eq!(None, map.get(&5).0);
//     for i in 0..10 {
//         map.delete(&i);
//     }
//     dbg!(&mapbak);
//     eprintln!("{}", db.debug_graphviz());
//     for i in 0..10 {
//         assert_eq!(Some(i), mapbak.get(&i).0);
//     }
//     // assert_eq!(&map.mapping.root_hash(), [0; 32]);
// }
