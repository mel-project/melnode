// use rand::prelude::SliceRandom;

use rstest::*;
use crate::{Block, CoinData, CoinDataHeight, CoinID, DENOM_TMEL, melvm, MICRO_CONVERTER, SmtMapping, State, Transaction, TxKind, DENOM_NEWCOIN};
use crate::testing::fixtures::{genesis_state, tx_send_mel_from_seed_coin, SEND_MEL_AMOUNT};
use crate::testing::factory::*;
use crate::testing::utils::{random_valid_txx, fee_estimate, tx_create_token, filter_tx_outputs_by_pk, tx_deposit};
use tmelcrypt::{Ed25519PK, Ed25519SK};
use std::hash::Hash;

// Add fuzz params ranges for rstest (range of num swaps, diff liquidity, etc...)
#[rstest]
fn test_melswap_v2_simple(
    genesis_state: State,
    tx_send_mel_from_seed_coin: ((Ed25519PK, Ed25519SK), Transaction)
) {
    // Seal genesis state
    let sealed_genesis_state = genesis_state.seal(None);

    // Fund liq provider with mel
    let (keypair_liq_provider, tx_fund_liq_provider) = tx_send_mel_from_seed_coin;

    // Get coin data for liq prov
    let outputs = filter_tx_outputs_by_pk(&keypair_liq_provider.0, &tx_fund_liq_provider.outputs);
    let idx = outputs.first().unwrap().clone().0;

    // Verify correct amount was issued
    assert_eq!(idx, 0);
    let coin_data = outputs.first().unwrap().clone().1.clone();
    assert_eq!(coin_data.value, SEND_MEL_AMOUNT);
    let mel_amount = coin_data.value;

    // liquidity provider creates a tx for a new token
    let token_amount = 1_000_000_000;
    let coin_id = tx_fund_liq_provider.get_coinid(idx);
    let tx_liq_prov_create_token = tx_create_token(&keypair_liq_provider, &coin_id, coin_data.value, token_amount);

    // We add that create token tx to the state
    let mut first_deposit_state = sealed_genesis_state.next_state();
    first_deposit_state.apply_tx(&tx_liq_prov_create_token);

    // Liquidity provider deposits mel/token pair
    // We use a 2:1 mel to token ratio on first deposit
    let mel_dep_amount = 2_000_000_000;

    // Check we are depositing l.t.e. to the amount of mel that liq provider has
    assert!( mel_amount >= mel_dep_amount);

    let tx_liq_prov_deposit = tx_deposit(&keypair_liq_provider, tx_liq_prov_create_token, token_amount, mel_dep_amount);
    first_deposit_state.apply_tx(&tx_liq_prov_deposit);

    // Seal the state for first deposit to start swapping for a set number of blocks
    let sealed_state = first_deposit_state.seal(None);

    // Create buyer and seller keypairs and fund them with mels and tokens
    let keypair_mel_buyer = tmelcrypt::ed25519_keygen();
    let keypair_mel_seller = tmelcrypt::ed25519_keygen();
    //
    // let (keypair_buyer, tx_fund_buyer) = tx_send_mels_to(keypair_liq_provider, keypair_mel_buyer, mel_amount);
    // let (keypair_buyer, tx_fund_buyer) = tx_send_mels_to(keypair_liq_provider, keypair_mel_buyer, mel_amount);
    //
    // let (keypair_liq_provider, tx_fund_liq_provider) = tx_send_tokens_from(keypair_liq_provider, token_amount);
    //
    // let (keypair_liq_provider, tx_fund_liq_provider) = tx_send_mel_from(keypair_liq_provider, mel_amount, token_amount);
    // //
    // let num_swapping_blocks = 100;
    //
    // for _ in 0..num_swapping_blocks {
    //     // let swapping_state = sealed_state.next_state();
    //
    //     // Create a mel buy swap tx with random amounts
    //
    //     // create a mel sull swap tx with random amounts
    //
    //     // apply tx
    //
    //     // seal block
    //
    //     // check liq_constant is expected
    // }
    //
    // // TODO: finish the rest of this (add more deposit) flow later...
    // // deposit more mel/tokens
    //
    // // swap for another M states
    //
    // // withdraw mel/tokens
    //
    // // swap for another O states which chekcing liq constant and price are correct
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
