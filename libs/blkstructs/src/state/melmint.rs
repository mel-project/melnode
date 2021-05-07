use num::{integer::Roots, rational::Ratio, BigInt, BigRational};
use std::convert::TryInto;

use super::melswap::PoolState;
use crate::{
    CoinData, CoinDataHeight, Denom, State, Transaction, TxKind, MAX_COINVAL, MICRO_CONVERTER,
};
use cached::proc_macro::cached;

/// Internal DOSC inflator. Returns how many µNomDOSC is 1 DOSC.
#[cached]
fn micronomdosc_per_dosc(height: u64) -> u128 {
    if height == 0 {
        MICRO_CONVERTER
    } else {
        // HACK: "segmented stacks"
        let last = if height % 1000 == 0 {
            std::thread::spawn(move || micronomdosc_per_dosc(height - 1))
                .join()
                .unwrap()
        } else {
            micronomdosc_per_dosc(height - 1)
        };
        (last + 1).max(last + last / 2_000_000)
    }
}

/// DOSC inflation ratio.
pub fn dosc_inflator(height: u64) -> BigRational {
    BigRational::from((
        BigInt::from(micronomdosc_per_dosc(height)),
        BigInt::from(MICRO_CONVERTER),
    ))
}

/// DOSC inflation calculator.
pub fn dosc_inflate_r2n(height: u64, real: u128) -> u128 {
    let ratio = dosc_inflator(height);
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
    let state = create_builtins(state);
    let state = process_swaps(state);
    let state = process_deposits(state);
    let state = process_withdrawals(state);
    process_pegging(state)
}

/// Creates the built-in pools if they don't exist. The built-in pools start out with nonzero liq, so that they can never be completely depleted. This ensures that built-in pools will always exist in the state.
fn create_builtins(mut state: State) -> State {
    let mut def = PoolState::new_empty();
    let _ = def.deposit(MICRO_CONVERTER * 1000, MICRO_CONVERTER * 1000);
    if state.pools.get(&Denom::Sym).0.is_none() {
        state.pools.insert(Denom::Sym, def)
    }
    if state.pools.get(&Denom::NomDosc).0.is_none() {
        state.pools.insert(Denom::NomDosc, def)
    }
    state
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
                && state
                    .pools
                    .get(&Denom::from_bytes(&tx.data).unwrap_or(Denom::NewCoin)) // Newcoin is something that never should appear
                    .0
                    .is_some()
                && (tx.outputs[0].denom == Denom::Mel || tx.outputs[0].denom.to_bytes() == tx.data)
        })
        .collect::<Vec<_>>();
    // find the pools mentioned
    let mut pools = swap_reqs
        .iter()
        .filter_map(|tx| Denom::from_bytes(&tx.data))
        .collect::<Vec<_>>();
    pools.sort_unstable();
    pools.dedup();
    // for each pool
    for pool in pools {
        let relevant_swaps: Vec<Transaction> = swap_reqs
            .iter()
            .filter(|tx| Denom::from_bytes(&tx.data) == Some(pool))
            .cloned()
            .collect();
        let mut pool_state = state.pools.get(&pool).0.unwrap();
        // sum up total mels and toks
        let total_mels = relevant_swaps
            .iter()
            .map(|tx| {
                if tx.outputs[0].denom == Denom::Mel {
                    tx.outputs[0].value
                } else {
                    0
                }
            })
            .fold(0u128, |a, b| a.saturating_add(b));
        let total_toks = relevant_swaps
            .iter()
            .map(|tx| {
                if tx.outputs[0].denom == Denom::Mel {
                    0
                } else {
                    tx.outputs[0].value
                }
            })
            .fold(0u128, |a, b| a.saturating_add(b));
        // transmute coins
        let (mel_withdrawn, tok_withdrawn) = pool_state.swap_many(total_mels, total_toks);
        for mut swap in relevant_swaps {
            if swap.outputs[0].denom == Denom::Mel {
                swap.outputs[0].denom = pool;
                swap.outputs[0].value =
                    multiply_frac(tok_withdrawn, Ratio::new(swap.outputs[0].value, total_mels))
                        .min(MAX_COINVAL);
            } else {
                swap.outputs[0].denom = Denom::Mel;
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
                && tx.outputs.len() >= 2
                && state.coins.get(&tx.get_coinid(0)).0.is_some()
                && state.coins.get(&tx.get_coinid(1)).0.is_some()
                && (tx.outputs[0].denom == Denom::Mel && tx.outputs[1].denom.to_bytes() == tx.data)
        })
        .collect::<Vec<_>>();
    // eprintln!("{} deposit reqs", deposit_reqs.len());
    // find the pools mentioned
    let pools = deposit_reqs
        .iter()
        .filter_map(|tx| Denom::from_bytes(&tx.data))
        .collect::<Vec<_>>();
    for pool in pools {
        let relevant_txx: Vec<Transaction> = deposit_reqs
            .iter()
            .filter(|tx| Denom::from_bytes(&tx.data) == Some(pool))
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
            state.pools.insert(pool, pool_state);
            liq
        } else {
            let mut pool_state = PoolState::new_empty();
            let liq = pool_state.deposit(total_mels, total_toks);
            state.pools.insert(pool, pool_state);
            liq
        };
        // divvy up the liqs
        for mut deposit in relevant_txx {
            let my_mtsqrt = deposit.outputs[0]
                .value
                .sqrt()
                .saturating_mul(deposit.outputs[1].value.sqrt());
            deposit.outputs[0].denom = liq_token_denom(pool);
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
                && state
                    .pools
                    .get(&Denom::from_bytes(&tx.data).unwrap_or(Denom::NewCoin))
                    .0
                    .is_some()
                && (tx.outputs[0].denom
                    == liq_token_denom(Denom::from_bytes(&tx.data).unwrap_or(Denom::NewCoin)))
        })
        .collect::<Vec<_>>();
    // find the pools mentioned
    let pools = withdraw_reqs
        .iter()
        .filter_map(|tx| Denom::from_bytes(&tx.data))
        .collect::<Vec<_>>();
    for pool in pools {
        let relevant_txx: Vec<Transaction> = withdraw_reqs
            .iter()
            .filter(|tx| tx.data == pool.to_bytes())
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
        state.pools.insert(pool, pool_state);
        // divvy up the mel and tok
        for mut deposit in relevant_txx {
            let my_liqs = deposit.outputs[0].value;
            deposit.outputs[0].denom = Denom::Mel;
            deposit.outputs[0].value = multiply_frac(total_mel, Ratio::new(my_liqs, total_liqs));
            let synth = CoinData {
                denom: pool,
                value: multiply_frac(total_tok, Ratio::new(my_liqs, total_liqs)),
                covhash: deposit.outputs[0].covhash,
                additional_data: deposit.outputs[0].additional_data.clone(),
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
    // first calculate the implied sym/nomDOSC exchange rate
    let x_s = state
        .pools
        .get(&Denom::Sym)
        .0
        .unwrap()
        .implied_price()
        .recip();
    let x_d = state
        .pools
        .get(&Denom::NomDosc)
        .0
        .unwrap()
        .implied_price()
        .recip();
    let r_sd = x_s / x_d;

    // get the right pool
    let mut sm_pool = state.pools.get(&Denom::Sym).0.unwrap();
    let konstant = BigInt::from(sm_pool.mels) * BigInt::from(sm_pool.tokens);
    // desired mel and sym
    let desired_r_sm = dosc_inflator(state.height) * r_sd;
    let desired_mel_sqr = BigRational::from(konstant.clone()) / desired_r_sm.clone();
    let desired_mel: u128 = desired_mel_sqr
        .floor()
        .numer()
        .sqrt()
        .try_into()
        .unwrap_or(u128::MAX);
    let desired_sym_sqr = BigRational::from(konstant) * desired_r_sm;
    let desired_sym: u128 = desired_sym_sqr
        .floor()
        .numer()
        .sqrt()
        .try_into()
        .unwrap_or(u128::MAX);
    // we nudge towards the desired level entirely through "normal" operations
    if desired_mel > sm_pool.mels {
        let delta = (desired_mel - sm_pool.mels) / 1000;
        // we increase mel liquidity by delta, throwing away the syms generated.
        // this nudges the exchange rate while minimizing long-term inflation
        let _ = sm_pool.swap_many(delta, 0);
    }
    if desired_sym > sm_pool.tokens {
        let delta = (desired_sym - sm_pool.tokens) / 1000;
        let _ = sm_pool.swap_many(0, delta);
    }
    state.pools.insert(Denom::Sym, sm_pool);
    // return the state now
    state
}

/// Denomination for a particular liquidity token
pub fn liq_token_denom(pool: Denom) -> Denom {
    Denom::Custom(tmelcrypt::hash_keyed(b"liq", pool.to_bytes()))
}

fn multiply_frac(x: u128, frac: Ratio<u128>) -> u128 {
    let frac = Ratio::new(BigInt::from(*frac.numer()), BigInt::from(*frac.denom()));
    let result = BigRational::from(BigInt::from(x)) * frac;
    result.floor().denom().try_into().unwrap_or(u128::MAX)
}

#[cfg(test)]
mod tests {
    use crate::{
        melvm,
        testing::fixtures::{genesis_mel_coin_id, genesis_state},
        CoinID, Denom,
    };

    use super::*;

    #[test]
    // test a simple deposit flow
    fn simple_deposit() {
        let (my_pk, my_sk) = tmelcrypt::ed25519_keygen();
        let my_covhash = melvm::Covenant::std_ed25519_pk_legacy(my_pk).hash();
        let start_state = genesis_state(
            CoinID::zero_zero(),
            CoinDataHeight {
                coin_data: CoinData {
                    value: 1 << 64,
                    denom: Denom::Mel,
                    covhash: my_covhash,
                    additional_data: vec![],
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
                    denom: Denom::Mel,
                    additional_data: vec![],
                },
                CoinData {
                    covhash: my_covhash,
                    value: 1 << 64,
                    denom: Denom::NewCoin,
                    additional_data: vec![],
                },
            ],
            fee: 2000000,
            scripts: vec![melvm::Covenant::std_ed25519_pk_legacy(my_pk)],
            data: vec![],
            sigs: vec![],
        }
        .signed_ed25519(my_sk);
        second_state.apply_tx(&newcoin_tx).unwrap();
        let deposit_tx = Transaction {
            kind: TxKind::LiqDeposit,
            inputs: vec![newcoin_tx.get_coinid(0), newcoin_tx.get_coinid(1)],
            outputs: vec![
                CoinData {
                    covhash: my_covhash,
                    value: (1 << 64) - 2000000 - 2000000,
                    denom: Denom::Mel,
                    additional_data: vec![],
                },
                CoinData {
                    covhash: my_covhash,
                    value: 1 << 64,
                    denom: Denom::Custom(newcoin_tx.hash_nosigs()),
                    additional_data: vec![],
                },
            ],
            fee: 2000000,
            scripts: vec![melvm::Covenant::std_ed25519_pk_legacy(my_pk)],
            data: newcoin_tx.hash_nosigs().to_vec(), // this is important, since it "points" to the pool
            sigs: vec![],
        }
        .signed_ed25519(my_sk);
        second_state.apply_tx(&deposit_tx).unwrap();
        let second_sealed = second_state.seal(None);
        for pool in second_sealed.inner_ref().pools.val_iter() {
            dbg!(pool);
        }
    }
}
