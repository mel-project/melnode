use crate::{Denom, SmtMapping, MICRO_CONVERTER};
use num::{rational::Ratio, BigInt, BigRational, BigUint};
use serde::{Deserialize, Serialize};
use std::convert::TryInto;

/// A pool
pub type PoolMapping = SmtMapping<Denom, PoolState>;

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct PoolState {
    pub mels: u128,
    pub tokens: u128,
    price_accum: u128,
    liqs: u128,
}

impl PoolState {
    /// Creates a new empty pool.
    pub fn new_empty() -> Self {
        Self {
            mels: 0,
            tokens: 0,
            price_accum: 0,
            liqs: 0,
        }
    }

    /// Executes a swap.
    #[must_use]
    pub fn swap_many(&mut self, mels: u128, tokens: u128) -> (u128, u128) {
        // deposit the tokens. intentionally saturate so that "overflowing" tokens are drained.
        self.mels = self.mels.saturating_add(mels);
        self.tokens = self.tokens.saturating_add(tokens);
        // "indiscriminately" use this new price to calculate how much of the other token to withdraw.
        let exchange_rate = Ratio::new(BigInt::from(self.mels), BigInt::from(self.tokens));
        let tok_to_withdraw: u128 = (BigRational::from(BigInt::from(mels)) / exchange_rate.clone()
            * BigRational::from(BigInt::from(995))
            / BigRational::from(BigInt::from(1000)))
        .floor()
        .numer()
        .try_into()
        .unwrap_or(u128::MAX);
        let mel_to_withdraw: u128 = (BigRational::from(BigInt::from(tokens))
            * exchange_rate
            * BigRational::from(BigInt::from(995))
            / BigRational::from(BigInt::from(1000)))
        .floor()
        .numer()
        .try_into()
        .unwrap_or(u128::MAX);
        // do the withdrawal
        self.mels -= mel_to_withdraw;
        self.tokens -= tok_to_withdraw;

        self.price_accum = self
            .price_accum
            .overflowing_add((self.mels).saturating_mul(MICRO_CONVERTER) / (self.tokens))
            .0;

        (mel_to_withdraw, tok_to_withdraw)
    }

    /// Deposits a set amount into the state, returning how many liquidity tokens were created.
    #[must_use]
    pub fn deposit(&mut self, mels: u128, tokens: u128) -> u128 {
        if self.liqs == 0 {
            self.mels = mels;
            self.tokens = tokens;
            self.liqs = mels;
            mels
        } else {
            // we first truncate mels and tokens because they can't overflow the state
            let mels = mels.saturating_add(self.mels) - self.mels;
            let tokens = tokens.saturating_add(self.tokens) - self.tokens;

            let delta_l_squared = (BigRational::from(BigInt::from(self.liqs).pow(2))
                * Ratio::new(
                    BigInt::from(mels) * BigInt::from(tokens),
                    BigInt::from(self.mels) * BigInt::from(self.tokens),
                ))
            .floor()
            .numer()
            .clone();
            let delta_l = delta_l_squared.sqrt();
            let delta_l = delta_l
                .to_biguint()
                .expect("deltaL can't possibly be negative");
            // we first convert deltaL to a u128, saturating on overflow
            let delta_l: u128 = delta_l.try_into().unwrap_or(u128::MAX);
            self.liqs = self.liqs.saturating_add(delta_l);
            self.mels += mels;
            self.tokens += tokens;
            // now we return
            delta_l
        }
    }

    /// Redeems a set amount of liquidity tokens, returning mels and tokens.
    #[must_use]
    pub fn withdraw(&mut self, liqs: u128) -> (u128, u128) {
        assert!(self.liqs >= liqs);
        let withdrawn_fraction = Ratio::new(BigUint::from(liqs), BigUint::from(self.liqs));
        let mels =
            Ratio::new(BigUint::from(self.mels), BigUint::from(1u32)) * withdrawn_fraction.clone();
        let toks = Ratio::new(BigUint::from(self.tokens), BigUint::from(1u32)) * withdrawn_fraction;
        self.liqs -= liqs;
        if self.liqs == 0 {
            let toret = (self.mels, self.tokens);
            self.mels = 0;
            self.tokens = 0;
            toret
        } else {
            let toret = (
                mels.floor().numer().try_into().unwrap(),
                toks.floor().numer().try_into().unwrap(),
            );
            self.mels -= toret.0;
            self.tokens -= toret.1;
            toret
        }
    }

    /// Returns the implied price as a fraction.
    #[must_use]
    pub fn implied_price(&self) -> BigRational {
        Ratio::new(BigInt::from(self.mels), BigInt::from(self.tokens))
    }
    /// Returns the liquidity constant of the system.
    #[must_use]
    pub fn liq_constant(&self) -> u128 {
        self.mels.saturating_mul(self.tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general() {
        let mut pool = PoolState::new_empty();
        let _ = pool.deposit(634684496, 1579230128);
        for _ in 1..5 {
            let out = pool.swap_many(100, 0);
            dbg!(pool);
            dbg!(pool.liq_constant());
            dbg!(out);
        }
    }
}
