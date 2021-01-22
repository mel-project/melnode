use crate::{CoinData, CoinID, State, Transaction, TxKind, COINTYPE_TSYM, MICRO_CONVERTER};
use std::convert::TryInto;

/// Synthesizes the auction-fill transaction, given a state.
pub fn synthesize_afill(state: &mut State) {
    // get inputs
    let inputs = state
        .auction_bids
        .val_iter()
        .map(|val| CoinID {
            txhash: val.hash_nosigs(),
            index: 0,
        })
        .collect();
    // find the auction winner
    let auction_winner = state
        .auction_bids
        .val_iter()
        .max_by(|x, y| x.outputs[0].value.cmp(&y.outputs[0].value));
    // for all the people who didn't win, return their money
    let mut outputs: Vec<_> = state
        .auction_bids
        .val_iter()
        .filter(|tx| tx.hash_nosigs() != auction_winner.as_ref().unwrap().hash_nosigs())
        .map(|tx| CoinData {
            conshash: tmelcrypt::HashVal(tx.data.clone().try_into().unwrap()),
            value: tx.outputs[0].value,
            cointype: tx.outputs[0].cointype.clone(),
        })
        .collect();
    if let Some(winner) = &auction_winner {
        let chash: [u8; 32] = winner.data.clone().try_into().unwrap();
        let chash = tmelcrypt::HashVal(chash);
        let output = CoinData {
            conshash: chash,
            value: MICRO_CONVERTER,
            cointype: COINTYPE_TSYM.to_vec(),
        };
        outputs.push(output);
    }
    let synth_tx = Transaction {
        kind: TxKind::AuctionFill,
        inputs,
        outputs,
        fee: 0,
        scripts: vec![],
        data: vec![],
        sigs: vec![],
    };
    // set the variables
    if let Some(winner) = auction_winner {
        state.sym_price = winner.outputs[0].value
    }
    // clear the auction and insert the transaction
    state.auction_bids.clear();
    state
        .apply_tx(&synth_tx)
        .expect("auction fill transactions can never be invalid. this indicates a bug");
}
