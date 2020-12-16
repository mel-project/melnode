use crate::{CoinData, CoinID, State, Transaction, TxKind, COINTYPE_TSYM, MICRO_CONVERTER};
use std::convert::TryInto;

/// Synthesizes the auction-fill transaction, given a state.
pub fn synthesize_afill(state: &mut State) {
    // get inputs
    let auction_winner = state
        .auction_bids
        .val_iter()
        .max_by(|x, y| x.outputs[0].value.cmp(&y.outputs[0].value));
    let synth_tx = Transaction {
        kind: TxKind::AuctionFill,
        inputs: if let Some(winner) = &auction_winner {
            let input = CoinID {
                txhash: winner.hash_nosigs(),
                index: 0,
            };
            vec![input]
        } else {
            vec![]
        },
        outputs: if let Some(winner) = &auction_winner {
            let chash: [u8; 32] = winner.data.clone().try_into().unwrap();
            let chash = tmelcrypt::HashVal(chash);
            let output = CoinData {
                conshash: chash,
                value: MICRO_CONVERTER,
                cointype: COINTYPE_TSYM.to_vec(),
            };
            vec![output]
        } else {
            vec![]
        },
        fee: 0,
        scripts: vec![],
        data: vec![],
        sigs: vec![],
    };
    // set the variables
    if let Some(winner) = auction_winner {
        state.met_price = winner.outputs[0].value
    }
    // clear the auction and insert the transaction
    state.auction_bids.clear();
    state
        .apply_tx(&synth_tx)
        .expect("auction fill transactions can never be invalid. this indicates a bug");
}
