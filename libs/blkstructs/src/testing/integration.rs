// use rand::prelude::SliceRandom;

// use crate::testing::factory::*;
use crate::testing::utils::{
    filter_tx_outputs_by_pk, random_valid_txx, tx_create_token, tx_deposit, tx_send_mels_to,
};
use crate::{melvm, CoinData, CoinID, SmtMapping, State, Transaction, MICRO_CONVERTER};
use crate::{
    testing::fixtures::{genesis_state, tx_send_mel_from_seed_coin, SEND_MEL_AMOUNT},
    Denom,
};
use rstest::*;
use tmelcrypt::{Ed25519PK, Ed25519SK};

// Add fuzz params ranges for rstest (range of num swaps, diff liquidity, etc...)
#[rstest]
fn test_melswap_v2_simple(
    genesis_state: State,
    tx_send_mel_from_seed_coin: ((Ed25519PK, Ed25519SK), Transaction),
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
    let tx_liq_prov_create_token = tx_create_token(
        &keypair_liq_provider,
        &coin_id,
        coin_data.value,
        token_amount,
    );

    // We add that create token tx to the state
    let mut first_deposit_state = sealed_genesis_state.next_state();
    first_deposit_state.apply_tx(&tx_fund_liq_provider).unwrap();
    first_deposit_state
        .apply_tx(&tx_liq_prov_create_token)
        .unwrap();

    // Liquidity provider deposits mel/token pair
    let mel_dep_amount = tx_liq_prov_create_token.outputs[0].value;

    // Check we are depositing l.t.e. to the amount of mel that liq provider has
    assert!(mel_amount >= mel_dep_amount);
    dbg!(mel_amount);

    let tx_liq_prov_deposit = tx_deposit(
        &keypair_liq_provider,
        tx_liq_prov_create_token.clone(),
        token_amount,
        mel_dep_amount,
    );
    first_deposit_state.apply_tx(&tx_liq_prov_deposit).unwrap();

    // Seal the state for first deposit to start swapping for a set number of blocks
    let sealed_state = first_deposit_state.seal(None);

    // Create buyer and seller keypairs and fund them with mels (to pay fees)
    // The buyer should buy buying mels and the seller should be selling mels only tokens
    let keypair_mel_buyer = tmelcrypt::ed25519_keygen();
    let _keypair_mel_seller = tmelcrypt::ed25519_keygen();

    let coin_id = tx_liq_prov_create_token.get_coinid(1);
    let tx_fund_buyer = tx_send_mels_to(
        &keypair_liq_provider,
        coin_id,
        keypair_mel_buyer.0,
        mel_amount,
        SEND_MEL_AMOUNT,
    );

    // get cid from prior tx
    let coin_id = tx_fund_buyer.get_coinid(1);
    let tx_fund_seller = tx_send_mels_to(
        &keypair_liq_provider,
        coin_id,
        keypair_mel_buyer.0,
        mel_amount,
        SEND_MEL_AMOUNT,
    );

    let _num_swapping_blocks = 1;

    // // Go to next state
    // let mut pre_swap_state = sealed_state.next_state();
    // pre_swap_state.apply_tx(&tx_fund_buyer).unwrap();
    // pre_swap_state.apply_tx(&tx_fund_seller).unwrap();

    // let sealed_state = pre_swap_state.seal(None);

    // let mut swapping_state = sealed_state.next_state();

    // // let expected_liq_constant = mel_dep_amount.mul(token_amount);

    // for _ in 0..num_swapping_blocks {
    //     // Do random buy and sell swaps
    //     let buy_amt = 10;
    //     let sell_amt = 20;
    //     let coin_id = tx_fund_buyer.get_coinid(0);
    //     let mel_buy_tx = create_mel_buy_tx(
    //         &keypair_mel_buyer,
    //         coin_id,
    //         tx_liq_prov_create_token.hash_nosigs(),
    //         buy_amt,
    //         sell_amt,
    //     );
    //     swapping_state.apply_tx(&mel_buy_tx);

    //     // let mel_sell_tx = create_mel_sell_tx(&keypair_mel_seller, amt);
    //     // swapping_state.apply_tx(&mel_sell_tx);

    //     // seal block
    //     let sealed_state = swapping_state.seal(None);

    //     swapping_state = sealed_state.next_state();

    //     // Manually examine pool contents
    //     for pool in sealed_state.inner_ref().pools.val_iter() {
    //         dbg!(pool);
    //     }
    //     // let actual_liq_constant = pool_state.liq_constant();
    //     // let expected_liq_constant = 1000000;
    //     // assert_eq!(expected_liq_constant, actual_liq_constant);
    // }

    // println!("hi");
    // // check liq_constant is expected (key is token denom)
    // let key = tx_liq_prov_create_token.hash_nosigs().to_vec();
    // let key2 = DENOM_NEWCOIN.to_vec();
    // let (pool_state, _proof) = swapping_state.pools.get(&key);
    // let (pool_state_2, _proof_2) = swapping_state.pools.get(&key2);
    // println!("HI");

    // // The goal is to  enrich the flow into real use cases
    // // deposit more mel/tokens
    // //
    // // swap for another M states
    // //
    // // withdraw some of the liquidity from mel/tokens pair
    // //
    // // swap for another O states which chekcing liq constant and price are correct
}

#[test]
fn state_simple_order_independence() {
    let db = novasmt::Forest::new(novasmt::InMemoryBackend::default());
    let (pk, sk) = tmelcrypt::ed25519_keygen();
    let scr = melvm::Covenant::std_ed25519_pk_legacy(pk);
    let mut genesis = State::test_genesis(db, MICRO_CONVERTER * 1000, scr.hash(), &[]);
    genesis.fee_multiplier = 0;
    let first_block = genesis.seal(None);
    let mut trng = rand::thread_rng();
    let txx = random_valid_txx(
        &mut trng,
        CoinID {
            txhash: tmelcrypt::HashVal([0; 32]),
            index: 0,
        },
        CoinData {
            covhash: scr.hash(),
            value: MICRO_CONVERTER * 1000,
            denom: Denom::Mel,
            additional_data: vec![],
        },
        sk,
        &scr,
        1577000,
    );
    println!("transactions generated");
    let _seq_copy = {
        let mut state = first_block.next_state();
        for tx in txx.iter() {
            state.apply_tx(tx).expect("failed application");
        }
        dbg!(state.seal(None).header()).hash()
    };
    // let copies: Vec<tmelcrypt::HashVal> = (0..8)
    //     .map(|_i| {
    //         let mut state = first_block.next_state();
    //         txx.shuffle(&mut trng);
    //         state.apply_tx_batch(&txx).expect("failed application");
    //         state.seal(None).header().hash()
    //     })
    //     .collect();
    // for c in copies {
    //     assert_eq!(c, seq_copy);
    // }
}

// TODO: Create an integration/smp_mapping.rs integration test and move this there.
#[test]
fn smt_mapping() {
    let db = novasmt::Forest::new(novasmt::InMemoryBackend::default());
    let tree = db.open_tree(Default::default()).unwrap();
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
    // eprintln!("{}", db.debug_graphviz());
    for i in 0..10 {
        assert_eq!(Some(i), mapbak.get(&i).0);
    }
    // assert_eq!(&map.mapping.root_hash(), [0; 32]);
}
