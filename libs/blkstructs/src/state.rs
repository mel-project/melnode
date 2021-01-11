use crate::constants::*;
use crate::smtmapping::*;
pub use crate::stake::*;
use crate::transaction as txn;
use defmac::defmac;
use parking_lot::RwLock;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use std::fmt::Debug;
use std::io::Read;
use std::sync::Arc;
use thiserror::Error;
use tmelcrypt::HashVal;
use txn::Transaction;
mod helpers;
mod melmint;

#[derive(Error, Debug)]
/// A error that happens while applying a transaction to a state
pub enum TxApplicationError {
    #[error("malformed transaction")]
    MalformedTx,
    #[error("attempted to spend non-existent coin {:?}", .0)]
    NonexistentCoin(txn::CoinID),
    #[error("unbalanced inputs and outputs")]
    UnbalancedInOut,
    #[error("referenced non-existent script {:?}", .0)]
    NonexistentScript(tmelcrypt::HashVal),
    #[error("does not satisfy script {:?}", .0)]
    ViolatesScript(tmelcrypt::HashVal),
    #[error("invalid sequential proof of work")]
    InvalidMelPoW,
    #[error("auction bid at wrong time")]
    BidWrongTime,
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
                    mets_staked: 100,
                },
            );
        }
        empty
    }
    /// Applies a single transaction.
    pub fn apply_tx(&mut self, tx: &txn::Transaction) -> Result<(), TxApplicationError> {
        self.apply_tx_batch(std::slice::from_ref(tx))
    }

    /// Applies a batch of transactions. The order of the transactions in txx do not matter.
    pub fn apply_tx_batch(&mut self, txx: &[txn::Transaction]) -> Result<(), TxApplicationError> {
        // clone self first
        let mut newself = self.clone();
        // first ensure that all the transactions are well-formed
        for tx in txx {
            if !tx.is_well_formed() {
                return Err(TxApplicationError::MalformedTx);
            }
            newself.transactions.insert(tx.hash_nosigs(), tx.clone());
        }
        let lnewself = RwLock::new(newself);
        // then we apply the outputs in parallel
        txx.par_iter()
            .for_each(|tx| helpers::apply_tx_outputs(&lnewself, tx));
        // then we apply the inputs in parallel
        let res: Result<Vec<()>, TxApplicationError> = txx
            .par_iter()
            .map(|tx| helpers::apply_tx_inputs(&lnewself, tx))
            .collect();
        res?;
        // then we apply the nondefault checks in parallel
        let res: Result<Vec<()>, TxApplicationError> = txx
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
    pub fn seal(self) -> SealedState {
        // create the finalized state
        SealedState(Arc::new(self))
    }

    // ----------- helpers start here ------------

    fn new_empty(db: autosmt::DBManager) -> Self {
        let empty_tree = db.get_tree(tmelcrypt::HashVal::default());
        State {
            height: 0,
            history: SmtMapping::new(empty_tree.clone()),
            coins: SmtMapping::new(empty_tree.clone()),
            transactions: SmtMapping::new(empty_tree.clone()),
            fee_pool: 1000000,
            fee_multiplier: 1000,
            dosc_multiplier: 1,
            auction_bids: SmtMapping::new(empty_tree.clone()),
            met_price: MICRO_CONVERTER,
            mel_price: MICRO_CONVERTER,
            stakes: SmtMapping::new(empty_tree),
        }
    }
}

/// SealedState represents an immutable state at a finalized block height. It cannot be constructed except through sealiong a State or restoring from persistent storage.
#[derive(Clone, Debug)]
pub struct SealedState(Arc<State>);

impl SealedState {
    /// Returns a reference to the State finalized within.
    pub fn inner_ref(&self) -> &State {
        &self.0
    }
    /// Partial encoding.
    pub fn partial_encoding(&self) -> Vec<u8> {
        self.0.partial_encoding()
    }
    /// Partial encoding.
    pub fn from_partial_encoding_infallible(bts: &[u8], db: &autosmt::DBManager) -> Self {
        SealedState(Arc::new(State::from_partial_encoding_infallible(bts, db)))
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
        let mut txx = Vec::new();
        for tx in self.0.transactions.val_iter() {
            txx.push(tx);
        }
        Block {
            header: self.header(),
            transactions: txx,
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

    /// Confirms a state with a given consensus proof. This function is supposed to be called to *verify* the consensus proof; `ConfirmedState`s cannot be constructed without checking the consensus proof as a result.
    ///
    /// **TODO**: Right now it DOES NOT check the consensus proof!
    pub fn confirm(
        self,
        cproof: symphonia::QuorumCert,
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

/// ConfirmedState represents a fully confirmed state with a consensus proof.
#[derive(Clone, Debug)]
pub struct ConfirmedState {
    state: SealedState,
    cproof: symphonia::QuorumCert,
}

impl ConfirmedState {
    /// Returns the wrapped finalized state
    pub fn inner(&self) -> &SealedState {
        &self.state
    }

    /// Returns the proof
    pub fn cproof(&self) -> &symphonia::QuorumCert {
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
        _cproof: &symphonia::QuorumCert,
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
    pub transactions: Vec<Transaction>,
}
