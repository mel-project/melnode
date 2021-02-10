use num::{integer::Roots, rational::Ratio, traits::Pow, BigInt, BigRational};
use std::convert::TryInto;

use crate::{
    CoinData, CoinDataHeight, State, Transaction, TxKind, DENOM_DOSC, DENOM_TMEL, DENOM_TSYM,
    MAX_COINVAL,
};

use super::melswap::PoolState;

/// DOSC inflation ratio.
pub fn dosc_inflator(height: u64) -> BigRational {
    BigRational::from((BigInt::from(10000005), BigInt::from(10000000))).pow(height)
}

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

/// Presealing function that is called before a state is sealed to apply melmint actions.
pub fn preseal_melmint(state: State) -> State {
    let state = process_swaps(state);
    let state = process_deposits(state);
    let state = process_withdrawals(state);
    process_pegging(state)
}

/// Process swaps.
fn process_swaps(mut state: State) -> State {
    // find the swap requests
    let swap_reqs = state
        .transactions
        .val_iter()
        .filter(|tx| {
            tx.kind == TxKind::Swap
                && !tx.outputs.is_empty()
                && state.coins.get(&tx.get_coinid(0)).0.is_some()
                && state.pools.get(&tx.data).0.is_some()
                && (tx.outputs[0].denom == DENOM_TMEL || tx.outputs[0].denom == tx.data)
        })
        .collect::<Vec<_>>();
    // find the pools mentioned
    let mut pools = swap_reqs
        .iter()
        .map(|tx| tx.data.clone())
        .collect::<Vec<_>>();
    pools.sort_unstable();
    pools.dedup();
    // for each pool
    for pool in pools {
        let relevant_swaps: Vec<Transaction> = swap_reqs
            .iter()
            .filter(|tx| tx.data == pool)
            .cloned()
            .collect();
        let mut pool_state = state.pools.get(&pool).0.unwrap();
        // sum up total mels and toks
        let total_mels = relevant_swaps
            .iter()
            .map(|tx| {
                if tx.outputs[0].denom == DENOM_TMEL {
                    tx.outputs[0].value
                } else {
                    0
                }
            })
            .fold(0u128, |a, b| a.saturating_add(b));
        let total_toks = relevant_swaps
            .iter()
            .map(|tx| {
                if tx.outputs[0].denom == DENOM_TMEL {
                    0
                } else {
                    tx.outputs[0].value
                }
            })
            .fold(0u128, |a, b| a.saturating_add(b));
        // transmute coins
        let (mel_withdrawn, tok_withdrawn) = pool_state.swap_many(total_mels, total_toks);
        for mut swap in relevant_swaps {
            if swap.outputs[0].denom == DENOM_TMEL {
                swap.outputs[0].denom = pool.clone();
                swap.outputs[0].value =
                    multiply_frac(tok_withdrawn, Ratio::new(swap.outputs[0].value, total_mels))
                        .min(MAX_COINVAL);
            } else {
                swap.outputs[0].denom = DENOM_TMEL.to_vec();
                swap.outputs[0].value =
                    multiply_frac(mel_withdrawn, Ratio::new(swap.outputs[0].value, total_toks))
                        .min(MAX_COINVAL);
            }
            state.coins.insert(
                swap.get_coinid(0),
                CoinDataHeight {
                    coin_data: swap.outputs[0].clone(),
                    height: state.height,
                },
            );
        }
        state.pools.insert(pool, pool_state);
    }
    state
}

/// Process deposits.
fn process_deposits(mut state: State) -> State {
    // find the deposit requests
    let deposit_reqs = state
        .transactions
        .val_iter()
        .filter(|tx| {
            tx.kind == TxKind::LiqDeposit
                && tx.outputs.len() > 2
                && state.coins.get(&tx.get_coinid(0)).0.is_some()
                && state.coins.get(&tx.get_coinid(1)).0.is_some()
                && (tx.outputs[0].denom == DENOM_TMEL && tx.outputs[1].denom == tx.data)
        })
        .collect::<Vec<_>>();
    // find the pools mentioned
    let pools = deposit_reqs
        .iter()
        .map(|tx| tx.data.clone())
        .collect::<Vec<_>>();
    for pool in pools {
        let relevant_txx: Vec<Transaction> = deposit_reqs
            .iter()
            .filter(|tx| tx.data == pool)
            .cloned()
            .collect();
        // sum up total mels and toks
        let total_mels = relevant_txx
            .iter()
            .map(|tx| tx.outputs[0].value)
            .fold(0u128, |a, b| a.saturating_add(b));
        let total_toks = relevant_txx
            .iter()
            .map(|tx| tx.outputs[1].value)
            .fold(0u128, |a, b| a.saturating_add(b));
        let total_mtsqrt = total_mels.sqrt().saturating_mul(total_toks.sqrt());
        // main logic here
        let total_liqs = if let Some(mut pool_state) = state.pools.get(&pool).0 {
            let liq = pool_state.deposit(total_mels, total_toks);
            state.pools.insert(pool.clone(), pool_state);
            liq
        } else {
            let mut pool_state = PoolState::new_empty();
            let liq = pool_state.deposit(total_mels, total_toks);
            state.pools.insert(pool.clone(), pool_state);
            liq
        };
        // divvy up the liqs
        for mut deposit in relevant_txx {
            let my_mtsqrt = deposit.outputs[0]
                .value
                .sqrt()
                .saturating_mul(deposit.outputs[1].value.sqrt());
            deposit.outputs[0].denom = liq_token_denom(&pool);
            deposit.outputs[0].value =
                multiply_frac(total_liqs, Ratio::new(my_mtsqrt, total_mtsqrt));
            state.coins.insert(
                deposit.get_coinid(0),
                CoinDataHeight {
                    coin_data: deposit.outputs[0].clone(),
                    height: state.height,
                },
            );
            state.coins.delete(&deposit.get_coinid(1));
        }
    }
    state
}

/// Process deposits.
fn process_withdrawals(mut state: State) -> State {
    // find the withdrawal requests
    let withdraw_reqs = state
        .transactions
        .val_iter()
        .filter(|tx| {
            tx.kind == TxKind::LiqWithdraw
                && tx.outputs.len() == 1
                && state.coins.get(&tx.get_coinid(0)).0.is_some()
                && state.pools.get(&tx.data).0.is_some()
                && (tx.outputs[0].denom == liq_token_denom(&tx.data))
        })
        .collect::<Vec<_>>();
    // find the pools mentioned
    let pools = withdraw_reqs
        .iter()
        .map(|tx| tx.data.clone())
        .collect::<Vec<_>>();
    for pool in pools {
        let relevant_txx: Vec<Transaction> = withdraw_reqs
            .iter()
            .filter(|tx| tx.data == pool)
            .cloned()
            .collect();
        // sum up total liqs
        let total_liqs = relevant_txx
            .iter()
            .map(|tx| tx.outputs[0].value)
            .fold(0u128, |a, b| a.saturating_add(b));
        // get the state
        let mut pool_state = state.pools.get(&pool).0.unwrap();
        let (total_mel, total_tok) = pool_state.withdraw(total_liqs);
        state.pools.insert(pool.clone(), pool_state);
        // divvy up the mel and tok
        for mut deposit in relevant_txx {
            let my_liqs = deposit.outputs[0].value;
            deposit.outputs[0].denom = DENOM_TMEL.to_vec();
            deposit.outputs[0].value = multiply_frac(total_mel, Ratio::new(my_liqs, total_liqs));
            let synth = CoinData {
                denom: pool.clone(),
                value: multiply_frac(total_tok, Ratio::new(my_liqs, total_liqs)),
                covhash: deposit.outputs[0].covhash,
            };

            state.coins.insert(
                deposit.get_coinid(0),
                CoinDataHeight {
                    coin_data: deposit.outputs[0].clone(),
                    height: state.height,
                },
            );
            state.coins.insert(
                deposit.get_coinid(1),
                CoinDataHeight {
                    coin_data: synth,
                    height: state.height,
                },
            );
        }
    }
    state
}

/// Process pegging.
fn process_pegging(mut state: State) -> State {
    if state.pools.get(&DENOM_TSYM.to_vec()).0.is_none()
        || state.pools.get(&DENOM_DOSC.to_vec()).0.is_none()
    {
        return state;
    }
    // first calculate the implied sym/nomDOSC exchange rate
    let x_s = state
        .pools
        .get(&DENOM_TSYM.to_vec())
        .0
        .unwrap()
        .implied_price()
        .recip();
    let x_d = state
        .pools
        .get(&DENOM_DOSC.to_vec())
        .0
        .unwrap()
        .implied_price()
        .recip();
    let r_sd = x_s / x_d;
    // we nudge the sym/mel exchange rate towards k*r_sd.
    let desired_r_sm = dosc_inflator(state.height) * r_sd;
    let mut sm_pool = state.pools.get(&DENOM_TSYM.to_vec()).0.unwrap();
    let stretched_sym = BigRational::from_float(0.9999).unwrap()
        * BigRational::from(BigInt::from(sm_pool.tokens))
        + BigRational::from_float(0.0001).unwrap()
            * BigRational::from(BigInt::from(sm_pool.mels))
            * desired_r_sm;
    let stretch_factor = stretched_sym.clone() / BigRational::from(BigInt::from(sm_pool.tokens));
    let new_sym_sqr: BigRational = stretched_sym.pow(2) / stretch_factor.clone();
    sm_pool.tokens = new_sym_sqr
        .floor()
        .numer()
        .sqrt()
        .try_into()
        .unwrap_or(u128::MAX);
    let new_mel_sqr: BigRational =
        BigRational::from(BigInt::from(sm_pool.mels)).pow(2) * stretch_factor;
    sm_pool.mels = new_mel_sqr
        .floor()
        .numer()
        .sqrt()
        .try_into()
        .unwrap_or(u128::MAX);
    state.pools.insert(DENOM_TSYM.to_vec(), sm_pool);
    // return the state now
    state
}

/// Denomination for a particular liquidity token
pub fn liq_token_denom(pool: &[u8]) -> Vec<u8> {
    tmelcrypt::hash_keyed(b"liq", pool).to_vec()
}

fn multiply_frac(x: u128, frac: Ratio<u128>) -> u128 {
    let frac = Ratio::new(BigInt::from(*frac.numer()), BigInt::from(*frac.denom()));
    let result = BigRational::from(BigInt::from(x)) * frac;
    result.floor().denom().try_into().unwrap_or(u128::MAX)
}

#[cfg(test)]
mod tests {
    use crate::{
        melscript,
        testing::fixtures::{genesis_mel_coin_id, genesis_state},
        CoinID, DENOM_DOSC, DENOM_NEWCOIN,
    };

    use super::*;

    #[test]
    fn simple_deposit() {
        let (my_pk, my_sk) = tmelcrypt::ed25519_keygen();
        let my_covhash = melscript::Script::std_ed25519_pk(my_pk).hash();
        let start_state = genesis_state(
            CoinID::zero_zero(),
            CoinDataHeight {
                coin_data: CoinData {
                    value: 1 << 64,
                    denom: DENOM_TMEL.to_vec(),
                    covhash: my_covhash,
                },
                height: 100,
            },
            Default::default(),
        );
        // test sealing
        let mut second_state = start_state.seal(None).next_state();
        // deposit the genesis as a custom-token pool
        let newcoin_tx = Transaction {
            kind: TxKind::Normal,
            inputs: vec![genesis_mel_coin_id()],
            outputs: vec![
                CoinData {
                    covhash: my_covhash,
                    value: (1 << 64) - 2000000,
                    denom: DENOM_TMEL.into(),
                },
                CoinData {
                    covhash: my_covhash,
                    value: 1 << 64,
                    denom: DENOM_NEWCOIN.into(),
                },
            ],
            fee: 2000000,
            scripts: vec![melscript::Script::std_ed25519_pk(my_pk)],
            data: vec![],
            sigs: vec![],
        }
        .sign_ed25519(my_sk);
        second_state.apply_tx(&newcoin_tx).unwrap();
    }
}
