use rand::prelude::SliceRandom;

use crate::{Block, CoinData, CoinDataHeight, CoinID, DENOM_TMEL, melscript, MICRO_CONVERTER, SmtMapping, State};
use crate::testing::fixtures::{genesis_mel_coin_id, genesis_state};
use crate::testing::utils::random_valid_txx;

#[test]
fn test_state_apply_block_with_deposits_and_swap(genesis_state: State, genesis_mel_coin_id: CoinID) {
    // let sealed_state = genesis_state.seal(None);
    // let transactions = transactions_factory(genesis_mel_coin_id);
    // let block = block_factory(sealed_state.header(), transactions);
    //
    // // insert deposit txs
    //
    // // Apply block
    // let sealed_state = sealed_state.apply_block(&block);
    //
    // let mut second_state = sealed_state.unwrap().next_state().
    // // test sealing
    // let mut second_state = .seal(None).next_state();

}

#[test]
fn state_simple_order_independence() {
    let db = autosmt::DBManager::load(autosmt::MemDB::default());
    let (pk, sk) = tmelcrypt::ed25519_keygen();
    let scr = melscript::Script::std_ed25519_pk(pk);
    let mut genesis = State::test_genesis(db, MICRO_CONVERTER * 1000, scr.hash(), &[]);
    genesis.fee_multiplier = 0;
    let first_block = genesis.seal(None);
    let mut trng = rand::thread_rng();
    let mut txx = random_valid_txx(
        &mut trng,
        CoinID {
            txhash: tmelcrypt::HashVal([0; 32]),
            index: 0,
        },
        CoinData {
            covhash: scr.hash(),
            value: MICRO_CONVERTER * 1000,
            denom: DENOM_TMEL.to_owned(),
        },
        sk,
        &scr,
        1577000
    );
    println!("transactions generated");
    let seq_copy = {
        let mut state = first_block.next_state();
        for tx in txx.iter() {
            state.apply_tx(tx).expect("failed application");
        }
        dbg!(state.seal(None).header()).hash()
    };
    let copies: Vec<tmelcrypt::HashVal> = (0..8)
        .map(|_i| {
            let mut state = first_block.next_state();
            txx.shuffle(&mut trng);
            state.apply_tx_batch(&txx).expect("failed application");
            state.seal(None).header().hash()
        })
        .collect();
    for c in copies {
        assert_eq!(c, seq_copy);
    }
}

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

fn test_deposit_tx() {

}