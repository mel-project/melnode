use std::convert::TryInto;

use dashmap::DashMap;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use tmelcrypt::HashVal;

use crate::{
    melvm::CovenantEnv, CoinDataHeight, CoinID, Denom, StakeDoc, State, StateError, Transaction,
    TxKind, COVHASH_DESTROY, STAKE_EPOCH,
};

use super::melmint;

/// A mutable "handle" to a particular State. Can be "committed" like a database transaction.
pub(crate) struct StateHandle<'a> {
    state: &'a mut State,

    coin_cache: DashMap<CoinID, Option<CoinDataHeight>>,
    transactions_cache: DashMap<HashVal, Transaction>,

    fee_pool_cache: u128,
    tips_cache: u128,

    stakes_cache: DashMap<HashVal, StakeDoc>,
}

impl<'a> StateHandle<'a> {
    /// Creates a new state handle.
    pub fn new(state: &'a mut State) -> Self {
        let fee_pool_cache = state.fee_pool;
        let tips_cache = state.tips;

        StateHandle {
            state,

            coin_cache: DashMap::new(),
            transactions_cache: DashMap::new(),

            fee_pool_cache,
            tips_cache,

            stakes_cache: DashMap::new(),
        }
    }

    /// Applies a batch of transactions, returning an error if any of them fail. Consumes and re-returns the handle; if any fail the handle is gone.
    pub fn apply_tx_batch(mut self, txx: &[Transaction]) -> Result<Self, StateError> {
        for tx in txx {
            if !tx.is_well_formed() {
                return Err(StateError::MalformedTx);
            }
            self.transactions_cache.insert(tx.hash_nosigs(), tx.clone());
            self.apply_tx_fees(tx)?;
        }
        // apply outputs in parallel
        txx.par_iter().for_each(|tx| self.apply_tx_outputs(tx));
        // apply inputs in parallel
        txx.par_iter()
            .map(|tx| self.apply_tx_inputs(tx))
            .collect::<Result<_, _>>()?;
        // apply specials in parallel
        txx.par_iter()
            .filter(|tx| tx.kind != TxKind::Normal && tx.kind != TxKind::Faucet)
            .map(|tx| self.apply_tx_special(tx))
            .collect::<Result<_, _>>()?;
        Ok(self)
    }

    /// Commits all the changes in this handle, at once.
    pub fn commit(self) {
        // commit coins
        for (k, v) in self.coin_cache {
            if let Some(v) = v {
                self.state.coins.insert(k, v);
            } else {
                self.state.coins.delete(&k);
            }
        }
        // commit txx
        for (k, v) in self.transactions_cache {
            self.state.transactions.insert(k, v);
        }
        // commit fees
        self.state.fee_pool = self.fee_pool_cache;
        self.state.tips = self.tips_cache;
        // commit stakes
        for (k, v) in self.stakes_cache {
            self.state.stakes.insert(k, v);
        }
    }

    fn apply_tx_inputs(&self, tx: &Transaction) -> Result<(), StateError> {
        let scripts = tx.script_as_map();
        // build a map of input coins
        let mut in_coins: im::HashMap<Denom, u128> = im::HashMap::new();
        // get last header
        let last_header = self
            .state
            .history
            .get(&(self.state.height.saturating_sub(1)))
            .0
            .unwrap_or_else(|| self.state.clone().seal(None).header());
        // iterate through the inputs
        for (spend_idx, coin_id) in tx.inputs.iter().enumerate() {
            if self.get_stake(coin_id.txhash).is_some() {
                return Err(StateError::CoinLocked);
            }
            let coin_data = self.get_coin(*coin_id);
            match coin_data {
                None => return Err(StateError::NonexistentCoin(*coin_id)),
                Some(coin_data) => {
                    log::trace!(
                        "coin_data {:?} => {:?} for txid {:?}",
                        coin_id,
                        coin_data,
                        tx.hash_nosigs()
                    );
                    let script = scripts
                        .get(&coin_data.coin_data.covhash)
                        .ok_or(StateError::NonexistentScript(coin_data.coin_data.covhash))?;
                    if !script.check(
                        tx,
                        CovenantEnv {
                            parent_coinid: coin_id,
                            parent_cdh: &coin_data,
                            spender_index: spend_idx as u8,
                            last_header: &last_header,
                        },
                    ) {
                        return Err(StateError::ViolatesScript(coin_data.coin_data.covhash));
                    }
                    self.del_coin(*coin_id);
                    in_coins.insert(
                        coin_data.coin_data.denom,
                        in_coins.get(&coin_data.coin_data.denom).unwrap_or(&0)
                            + coin_data.coin_data.value,
                    );
                }
            }
        }
        // balance inputs and outputs. ignore outputs with empty cointype (they create a new token kind)
        let out_coins = tx.total_outputs();
        if tx.kind != TxKind::Faucet {
            for (currency, value) in out_coins.iter() {
                // we skip the created doscs for a DoscMint transaction
                if tx.kind == TxKind::DoscMint && *currency == Denom::NomDosc {
                    continue;
                }
                if *currency != Denom::NewCoin
                    && *value != *in_coins.get(currency).unwrap_or(&u128::MAX)
                {
                    return Err(StateError::UnbalancedInOut);
                }
            }
        }
        Ok(())
    }

    fn apply_tx_fees(&mut self, tx: &Transaction) -> Result<(), StateError> {
        // fees
        let min_fee = tx.base_fee(self.state.fee_multiplier, 0);
        if tx.fee < min_fee {
            return Err(StateError::InsufficientFees(min_fee));
        }
        let tips = tx.fee - min_fee;
        self.tips_cache = self.tips_cache.saturating_add(tips);
        self.fee_pool_cache = self.fee_pool_cache.saturating_add(min_fee);
        Ok(())
    }

    fn apply_tx_outputs(&self, tx: &Transaction) {
        let height = self.state.height;
        for (index, coin_data) in tx.outputs.iter().enumerate() {
            let mut coin_data = coin_data.clone();
            if coin_data.denom == Denom::NewCoin {
                coin_data.denom = Denom::Custom(tx.hash_nosigs());
            }
            // if covenant hash is zero, this destroys the coins permanently
            if coin_data.covhash != COVHASH_DESTROY {
                self.set_coin(
                    CoinID {
                        txhash: tx.hash_nosigs(),
                        index: index.try_into().unwrap(),
                    },
                    CoinDataHeight { coin_data, height },
                );
            }
        }
    }

    fn apply_tx_special(&self, tx: &Transaction) -> Result<(), StateError> {
        match tx.kind {
            TxKind::DoscMint => self.apply_tx_special_doscmint(tx),
            TxKind::Stake => self.apply_tx_special_stake(tx),
            _ => Ok(()),
        }
    }

    fn apply_tx_special_doscmint(&self, tx: &Transaction) -> Result<(), StateError> {
        let coin_id = *tx.inputs.get(0).ok_or(StateError::MalformedTx)?;
        let coin_data = self.get_coin(coin_id).ok_or(StateError::MalformedTx)?;
        // make sure the time is long enough that we can easily measure it
        if self.state.height - coin_data.height < 100 {
            return Err(StateError::InvalidMelPoW);
        }
        // construct puzzle seed
        let chi = tmelcrypt::hash_keyed(
            &self.state.history.get(&coin_data.height).0.unwrap().hash(),
            &stdcode::serialize(tx.inputs.get(0).ok_or(StateError::MalformedTx)?).unwrap(),
        );
        // get difficulty and proof
        let (difficulty, proof): (u32, Vec<u8>) =
            stdcode::deserialize(&tx.data).map_err(|_| StateError::MalformedTx)?;
        let proof = melpow::Proof::from_bytes(&proof).ok_or(StateError::MalformedTx)?;
        if !proof.verify(&chi, difficulty as _) {
            return Err(StateError::InvalidMelPoW);
        }
        // compute speeds
        let my_speed = 2u128.pow(difficulty);
        let reward_real = melmint::calculate_reward(my_speed, self.state.dosc_speed, difficulty);
        let reward_nom = melmint::dosc_inflate_r2n(self.state.height, reward_real);
        // ensure that the total output of DOSCs is correct
        let total_dosc_output = tx
            .total_outputs()
            .get(&Denom::NomDosc)
            .cloned()
            .unwrap_or_default();
        if total_dosc_output > reward_nom {
            return Err(StateError::InvalidMelPoW);
        }
        Ok(())
    }
    fn apply_tx_special_stake(&self, tx: &Transaction) -> Result<(), StateError> {
        // first we check that the data is correct
        let stake_doc: StakeDoc =
            stdcode::deserialize(&tx.data).map_err(|_| StateError::MalformedTx)?;
        let curr_epoch = self.state.height / STAKE_EPOCH;
        // then we check that the first coin is valid
        let first_coin = tx.outputs.get(0).ok_or(StateError::MalformedTx)?;
        if first_coin.denom != Denom::Sym {
            return Err(StateError::MalformedTx);
        }
        // then we check consistency
        if !(stake_doc.e_start > curr_epoch
            && stake_doc.e_post_end > stake_doc.e_start
            && stake_doc.syms_staked == first_coin.value)
        {
            self.set_stake(tx.hash_nosigs(), stake_doc);
        }
        Ok(())
    }

    fn get_coin(&self, coin_id: CoinID) -> Option<CoinDataHeight> {
        self.coin_cache
            .entry(coin_id)
            .or_insert_with(|| self.state.coins.get(&coin_id).0)
            .value()
            .clone()
    }

    fn set_coin(&self, coin_id: CoinID, value: CoinDataHeight) {
        self.coin_cache.insert(coin_id, Some(value));
    }

    fn del_coin(&self, coin_id: CoinID) {
        self.coin_cache.insert(coin_id, None);
    }

    fn get_stake(&self, txhash: HashVal) -> Option<StakeDoc> {
        if let Some(cached_sd) = self.stakes_cache.get(&txhash).as_deref() {
            return Some(cached_sd).cloned();
        }
        if let Some(sd) = self.state.stakes.get(&txhash).0 {
            return self.stakes_cache.insert(txhash, sd);
        }
        None
    }

    fn set_stake(&self, txhash: HashVal, sdoc: StakeDoc) {
        self.stakes_cache.insert(txhash, sdoc);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    // use crate::melvm::Covenant;
    // use crate::state::applytx::StateHandle;
    // // use crate::testing::factory::*;
    // use crate::testing::fixtures::*;
    // use crate::{CoinData, CoinID, State};
    // use rstest::*;
    // use tmelcrypt::{Ed25519PK, Ed25519SK};
    //
    // #[rstest]
    // fn test_apply_tx_inputs_single_valid_tx(
    //     genesis_state: State,
    //     genesis_mel_coin_id: CoinID,
    //     genesis_mel_coin_data: CoinData,
    //     genesis_covenant_keypair: (Ed25519PK, Ed25519SK),
    //     genesis_covenant: Covenant,
    //     keypair: (Ed25519PK, Ed25519SK),
    // ) {
    //     // Init state and state handle
    //     let mut state = genesis_state.clone();
    //     let state_handle = StateHandle::new(&mut state);
    //
    //     // Create a valid signed transaction from first coin
    //     // let fee = 3000000;
    //     // let tx = tx_factory(
    //     //     TxKind::Normal,
    //     //     genesis_covenant_keypair,
    //     //     keypair.0,
    //     //     genesis_mel_coin_id,
    //     //     genesis_covenant,
    //     //     genesis_mel_coin_data.value,
    //     //     fee,
    //     // );
    //     //
    //     // // Apply tx inputs and verify no error
    //     // let res = state_handle.apply_tx_inputs(&tx);
    //     //
    //     // assert!(res.is_ok());
    // }
}
