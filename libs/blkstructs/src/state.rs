pub use crate::stake::*;
use crate::{constants::*, CoinDataHeight};
use crate::{smtmapping::*, CoinData};
use crate::{transaction as txn, CoinID};
use bytes::Bytes;
use defmac::defmac;
use parking_lot::RwLock;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::io::Read;
use std::sync::Arc;
use std::{collections::BTreeMap, convert::TryInto};
use thiserror::Error;
use tmelcrypt::{Ed25519PK, HashVal};
use txn::Transaction;
mod helpers;
mod melmint;

// TODO: Move these structs into state package
// ie: split this into modules such as
// error.rs header.rs seal.rs propser.rs block.rs state.rs and lib
// and put them into the state folder or rename state folder to blk folder

#[derive(Error, Debug)]
/// A error that happens while applying a transaction to a state
pub enum StateError {
    #[error("malformed transaction")]
    MalformedTx,
    #[error("attempted to spend non-existent coin {:?}", .0)]
    NonexistentCoin(txn::CoinID),
    #[error("unbalanced inputs and outputs")]
    UnbalancedInOut,
    #[error("insufficient fees (requires {0})")]
    InsufficientFees(u64),
    #[error("referenced non-existent script {:?}", .0)]
    NonexistentScript(tmelcrypt::HashVal),
    #[error("does not satisfy script {:?}", .0)]
    ViolatesScript(tmelcrypt::HashVal),
    #[error("invalid sequential proof of work")]
    InvalidMelPoW,
    #[error("auction bid at wrong time")]
    BidWrongTime,
    #[error("block has wrong header after applying to previous block")]
    WrongHeader,
}

/// World state of the Themelio blockchain
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct State {
    pub height: u64,
    pub history: SmtMapping<u64, Header>,
    pub coins: SmtMapping<txn::CoinID, txn::CoinDataHeight>,
    pub transactions: SmtMapping<HashVal, txn::Transaction>,

    pub fee_pool: u64,
    pub fee_multiplier: u64,
    pub tips: u64,

    pub dosc_multiplier: u64,
    pub auction_bids: SmtMapping<HashVal, txn::Transaction>,
    pub met_price: u64,
    pub mel_price: u64,

    pub stakes: SmtMapping<HashVal, StakeDoc>,
}

fn read_bts(r: &mut impl Read, n: usize) -> Option<Vec<u8>> {
    let mut buf: Vec<u8> = vec![0; n];
    r.read_exact(&mut buf).ok()?;
    Some(buf)
}

impl State {
    /// Generates an encoding of the state that, in conjuction with a SMT database, can recover the entire state.
    pub fn partial_encoding(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.height.to_be_bytes());
        out.extend_from_slice(&self.history.root_hash());
        out.extend_from_slice(&self.coins.root_hash());
        out.extend_from_slice(&self.transactions.root_hash());

        out.extend_from_slice(&self.fee_pool.to_be_bytes());
        out.extend_from_slice(&self.fee_multiplier.to_be_bytes());
        out.extend_from_slice(&self.tips.to_be_bytes());

        out.extend_from_slice(&self.dosc_multiplier.to_be_bytes());
        out.extend_from_slice(&self.auction_bids.root_hash());
        out.extend_from_slice(&self.met_price.to_be_bytes());
        out.extend_from_slice(&self.mel_price.to_be_bytes());

        out.extend_from_slice(&self.stakes.root_hash());
        out
    }

    /// Restores a state from its partial encoding in conjunction with a database. **Does not validate data and will panic; do not use on untrusted data**
    pub fn from_partial_encoding_infallible(mut encoding: &[u8], db: &autosmt::DBManager) -> Self {
        defmac!(readu64 => u64::from_be_bytes(read_bts(&mut encoding, 8).unwrap().as_slice().try_into().unwrap()));
        defmac!(readtree => SmtMapping::new(db.get_tree(tmelcrypt::HashVal(
            read_bts(&mut encoding, 32).unwrap().as_slice().try_into().unwrap(),
        ))));
        let height = readu64!();
        let history = readtree!();
        let coins = readtree!();
        let transactions = readtree!();

        let fee_pool = readu64!();
        let fee_multiplier = readu64!();
        let tips = readu64!();

        let dosc_multiplier = readu64!();
        let auction_bids = readtree!();
        let met_price = readu64!();
        let mel_price = readu64!();

        let stakes = readtree!();
        State {
            height,
            history,
            coins,
            transactions,

            fee_pool,
            fee_multiplier,
            tips,

            dosc_multiplier,
            auction_bids,
            met_price,
            mel_price,

            stakes,
        }
    }

    /// Generates a test genesis state, with a given starting coin.
    pub fn test_genesis(
        db: autosmt::DBManager,
        start_micromels: u64,
        start_conshash: tmelcrypt::HashVal,
        start_stakeholders: &[tmelcrypt::Ed25519PK],
    ) -> Self {
        assert!(start_micromels <= MAX_COINVAL);
        let mut empty = Self::new_empty(db);
        // insert coin out of nowhere
        let init_coin = txn::CoinData {
            conshash: start_conshash,
            value: start_micromels,
            cointype: COINTYPE_TMEL.to_vec(),
        };
        empty.coins.insert(
            txn::CoinID {
                txhash: tmelcrypt::HashVal([0; 32]),
                index: 0,
            },
            txn::CoinDataHeight {
                coin_data: init_coin,
                height: 0,
            },
        );
        for (i, stakeholder) in start_stakeholders.iter().enumerate() {
            empty.stakes.insert(
                tmelcrypt::hash_single(&(i as u64).to_be_bytes()),
                StakeDoc {
                    pubkey: *stakeholder,
                    e_start: 0,
                    e_post_end: 1000000000,
                    syms_staked: 100,
                },
            );
        }
        empty
    }
    /// Applies a single transaction.
    pub fn apply_tx(&mut self, tx: &txn::Transaction) -> Result<(), StateError> {
        self.apply_tx_batch(std::slice::from_ref(tx))
    }

    /// Applies a batch of transactions. The order of the transactions in txx do not matter.
    pub fn apply_tx_batch(&mut self, txx: &[txn::Transaction]) -> Result<(), StateError> {
        // clone self first
        let mut newself = self.clone();
        // first ensure that all the transactions are well-formed
        for tx in txx {
            if !tx.is_well_formed() {
                return Err(StateError::MalformedTx);
            }
            newself.transactions.insert(tx.hash_nosigs(), tx.clone());
        }
        let lnewself = RwLock::new(newself);
        // then we apply the outputs in parallel
        txx.par_iter()
            .for_each(|tx| helpers::apply_tx_outputs(&lnewself, tx));
        // then we apply the inputs in parallel
        let res: Result<Vec<()>, StateError> = txx
            .par_iter()
            .map(|tx| helpers::apply_tx_inputs(&lnewself, tx))
            .collect();
        res?;
        // then we apply the nondefault checks in parallel
        let res: Result<Vec<()>, StateError> = txx
            .par_iter()
            .filter(|tx| tx.kind != txn::TxKind::Normal && tx.kind != txn::TxKind::Faucet)
            .map(|tx| helpers::apply_tx_special(&lnewself, tx))
            .collect();
        res?;
        // we commit the changes
        //panic!("COMMIT?!");
        log::debug!(
            "applied a batch of {} txx to {:?} => {:?}",
            txx.len(),
            self.coins.root_hash(),
            lnewself.read().coins.root_hash()
        );
        *self = lnewself.read().clone();
        Ok(())
    }

    /// Finalizes a state into a block. This consumes the state.
    pub fn seal(mut self, action: Option<ProposerAction>) -> SealedState {
        // apply the proposer action
        if let Some(action) = action {
            // first let's move the fee multiplier
            let max_movement = (self.fee_multiplier >> 7) as i64;
            let scaled_movement = max_movement * action.fee_multiplier_delta as i64 / 128;
            log::debug!(
                "changing fee multiplier {} by {}",
                self.fee_multiplier,
                scaled_movement
            );
            if scaled_movement >= 0 {
                self.fee_multiplier += scaled_movement as u64;
            } else {
                self.fee_multiplier -= scaled_movement.abs() as u64;
            }
            // then it's time to collect the fees dude! we synthesize a coin with 1/65536 of the fee pool and all the tips.
            let base_fees = self.fee_pool >> 16;
            self.fee_pool -= base_fees;
            let tips = self.tips;
            self.tips = 0;
            let pseudocoin_id = reward_coin_pseudoid(self.height);
            let pseudocoin_data = CoinDataHeight {
                coin_data: CoinData {
                    conshash: action.reward_dest,
                    value: base_fees + tips,
                    cointype: COINTYPE_TMEL.into(),
                },
                height: self.height,
            };
            // insert the fake coin
            self.coins.insert(pseudocoin_id, pseudocoin_data);
        }
        // create the finalized state
        SealedState(Arc::new(self), action)
    }

    // ----------- helpers start here ------------

    pub(crate) fn new_empty(db: autosmt::DBManager) -> Self {
        let empty_tree = db.get_tree(tmelcrypt::HashVal::default());
        State {
            height: 0,
            history: SmtMapping::new(empty_tree.clone()),
            coins: SmtMapping::new(empty_tree.clone()),
            transactions: SmtMapping::new(empty_tree.clone()),
            fee_pool: 1000000,
            fee_multiplier: 1000,
            dosc_multiplier: 1,
            tips: 0,
            auction_bids: SmtMapping::new(empty_tree.clone()),
            met_price: MICRO_CONVERTER,
            mel_price: MICRO_CONVERTER,
            stakes: SmtMapping::new(empty_tree),
        }
    }
}

pub fn reward_coin_pseudoid(height: u64) -> CoinID {
    CoinID {
        txhash: tmelcrypt::hash_keyed(b"reward_coin_pseudoid", &height.to_be_bytes()),
        index: 0,
    }
}

/// SealedState represents an immutable state at a finalized block height. It cannot be constructed except through sealiong a State or restoring from persistent storage.
#[derive(Clone, Debug)]
pub struct SealedState(Arc<State>, Option<ProposerAction>);

impl SealedState {
    /// Returns a reference to the State finalized within.
    pub fn inner_ref(&self) -> &State {
        &self.0
    }
    /// Returns whether or not it's empty.
    pub fn is_empty(&self) -> bool {
        self.1.is_none() && self.inner_ref().transactions.root_hash() == Default::default()
    }
    /// Partial encoding.
    pub fn partial_encoding(&self) -> Vec<u8> {
        let tmp = (self.0.partial_encoding(), &self.1);
        bincode::serialize(&tmp).unwrap()
    }
    /// Partial encoding.
    pub fn from_partial_encoding_infallible(bts: &[u8], db: &autosmt::DBManager) -> Self {
        let tmp: (Vec<u8>, Option<ProposerAction>) = bincode::deserialize(&bts).unwrap();
        SealedState(
            Arc::new(State::from_partial_encoding_infallible(&tmp.0, db)),
            tmp.1,
        )
    }
    /// Returns the block header represented by the finalized state.
    pub fn header(&self) -> Header {
        let inner = &self.0;
        // panic!()
        Header {
            previous: (inner.height.checked_sub(1))
                .map(|height| inner.history.get(&height).0.unwrap().hash())
                .unwrap_or_default(),
            height: inner.height,
            history_hash: inner.history.root_hash(),
            coins_hash: inner.coins.root_hash(),
            transactions_hash: inner.transactions.root_hash(),
            fee_pool: inner.fee_pool,
            fee_multiplier: inner.fee_multiplier,
            dosc_multiplier: inner.dosc_multiplier,
            auction_bids_hash: inner.auction_bids.root_hash(),
            met_price: inner.met_price,
            mel_price: inner.mel_price,
            stake_doc_hash: inner.stakes.root_hash(),
        }
    }
    /// Returns the final state represented as a "block" (header + transactions).
    pub fn to_block(&self) -> Block {
        let mut txx = im::HashSet::new();
        for tx in self.0.transactions.val_iter() {
            txx.insert(tx);
        }
        Block {
            header: self.header(),
            transactions: txx,
            proposer_action: self.1,
        }
    }
    /// Creates a new unfinalized state representing the next block.
    pub fn next_state(&self) -> State {
        let mut new = self.inner_ref().clone();
        // advance the numbers
        new.history.insert(self.0.height, self.header());
        new.height += 1;
        new.stakes.remove_stale(new.height / STAKE_EPOCH);
        new.transactions.clear();
        // synthesize auction fill as needed
        if new.height % AUCTION_INTERVAL == 0 && !new.auction_bids.is_empty() {
            melmint::synthesize_afill(&mut new)
        }
        new
    }

    /// Applies a block to this state.
    pub fn apply_block(&self, block: &Block) -> Result<SealedState, StateError> {
        let mut basis = self.next_state();
        basis.apply_tx_batch(&block.transactions.iter().cloned().collect::<Vec<_>>())?;
        let basis = basis.seal(block.proposer_action);
        if basis.header() != block.header {
            return Err(StateError::WrongHeader);
        }
        Ok(basis)
    }

    /// Confirms a state with a given consensus proof. This function is supposed to be called to *verify* the consensus proof; `ConfirmedState`s cannot be constructed without checking the consensus proof as a result.
    ///
    /// **TODO**: Right now it DOES NOT check the consensus proof!
    pub fn confirm(
        self,
        cproof: ConsensusProof,
        previous_state: Option<&State>,
    ) -> Option<ConfirmedState> {
        if previous_state.is_none() {
            assert_eq!(self.inner_ref().height, 0);
        }
        Some(ConfirmedState {
            state: self,
            cproof,
        })
    }
}

/// ProposerAction describes the standard action that the proposer takes when proposing a block.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
pub struct ProposerAction {
    /// Change in fee. This is scaled to the proper size.
    pub fee_multiplier_delta: i8,
    /// Where to sweep fees.
    pub reward_dest: HashVal,
}

pub type ConsensusProof = BTreeMap<Ed25519PK, Bytes>;

/// ConfirmedState represents a fully confirmed state with a consensus proof.
#[derive(Clone, Debug)]
pub struct ConfirmedState {
    state: SealedState,
    cproof: ConsensusProof,
}

impl ConfirmedState {
    /// Returns the wrapped finalized state
    pub fn inner(&self) -> &SealedState {
        &self.state
    }

    /// Returns the proof
    pub fn cproof(&self) -> &ConsensusProof {
        &self.cproof
    }
}

// impl Deref<Target =

#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
/// A block header.
pub struct Header {
    pub previous: HashVal,
    pub height: u64,
    pub history_hash: HashVal,
    pub coins_hash: HashVal,
    pub transactions_hash: HashVal,
    pub fee_pool: u64,
    pub fee_multiplier: u64,
    pub dosc_multiplier: u64,
    pub auction_bids_hash: HashVal,
    pub met_price: u64,
    pub mel_price: u64,
    pub stake_doc_hash: HashVal,
}

impl Header {
    pub fn hash(&self) -> tmelcrypt::HashVal {
        tmelcrypt::hash_single(&bincode::serialize(self).unwrap())
    }

    pub fn validate_cproof(
        &self,
        _cproof: &ConsensusProof,
        previous_state: Option<&State>,
    ) -> bool {
        if previous_state.is_none() && self.height != 0 {
            return false;
        }
        // TODO
        true
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
/// A (serialized) block.
pub struct Block {
    pub header: Header,
    pub transactions: im::HashSet<Transaction>,
    pub proposer_action: Option<ProposerAction>,
}

impl Block {
    /// Abbreviate a block
    pub fn abbreviate(&self) -> AbbrBlock {
        AbbrBlock {
            header: self.header,
            txhashes: self.transactions.iter().map(|v| v.hash_nosigs()).collect(),
            proposer_action: self.proposer_action,
        }
    }
}

/// An abbreviated block
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AbbrBlock {
    pub header: Header,
    pub txhashes: im::HashSet<HashVal>,
    pub proposer_action: Option<ProposerAction>,
}


#[cfg(test)]
pub(crate) mod tests {
    use rstest::*;
    use crate::testing::fixtures::valid_txx;
    use crate::Transaction;

    fn test_apply_tx_batch_not_well_formed_errors() {
        // create a batch of transactions

        // ensure at least one of them is not well formed

        // call apply tx batch

        // verify you get a state error
    }

    #[rstest]
    fn test_apply_tx_batch(valid_txx: Vec<Transaction>) {
        // create a batch of transactions
        // valid_txx()
        // call apply tx batch

        // verify result is ok
    }

    // TODO: add tests for State::seal & SealedState methods
}