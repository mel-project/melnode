use crate::{CoinData, CoinID, State, Transaction, TxKind, DENOM_TSYM, MICRO_CONVERTER};
use num::{traits::Pow, BigInt, BigRational};
use std::convert::TryInto;

/// DOSC inflation calculator.
pub fn dosc_inflate_r2n(height: u64, real: u128) -> u128 {
    let ratio = BigRational::from((BigInt::from(10000005), BigInt::from(10000000))).pow(height);
    let result = ratio * BigRational::from(BigInt::from(real));
    result
        .floor()
        .numer()
        .to_biguint()
        .unwrap()
        .try_into()
        .expect("dosc inflated so much it doesn't fit into a u128")
}

/// Reward calculator. Returns the value in real DOSC.
pub fn calculate_reward(my_speed: u128, dosc_speed: u128, difficulty: u32) -> u128 {
    let exp_difficulty = 2u128.pow(difficulty as _);
    // correct calculation with bigints
    let result = (BigInt::from(exp_difficulty) * BigInt::from(my_speed)) / BigInt::from(dosc_speed);
    result.try_into().unwrap_or(u128::MAX)
}

/// Synthesizes the auction-fill transaction, given a state.
pub(crate) fn synthesize_afill(state: &mut State) {
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
            covhash: tmelcrypt::HashVal(tx.data.clone().try_into().unwrap()),
            value: tx.outputs[0].value,
            denom: tx.outputs[0].denom.clone(),
        })
        .collect();
    if let Some(winner) = &auction_winner {
        let chash: [u8; 32] = winner.data.clone().try_into().unwrap();
        let chash = tmelcrypt::HashVal(chash);
        let output = CoinData {
            covhash: chash,
            value: MICRO_CONVERTER,
            denom: DENOM_TSYM.to_vec(),
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

// #[cfg(test)]
// mod tests {
//     use super::dosc_inflate;

//     #[test]
//     fn inflation() {
//         for i in 0..10000 {
//             dbg!(dosc_inflate(i, 100000000));
//         }
//     }
// }
