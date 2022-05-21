use crate::storage::MeshaCas;

use std::{collections::HashSet, time::Instant};

use lru::LruCache;
use themelio_stf::{State, StateError};
use themelio_structs::{Transaction, TxHash};

/// Mempool encapsulates a "mempool" --- a provisional state that is used to form new blocks by stakers, or provisionally validate transactions by auditors.
pub struct Mempool {
    provisional_state: State<MeshaCas>,
    last_rebase: State<MeshaCas>,
    txx_in_state: HashSet<TxHash>,
    // seen: LruCache<TxHash, ()>,
}

impl Mempool {
    /// Create sa new mempool based on a provisional state.
    pub fn new(state: State<MeshaCas>) -> Self {
        Self {
            provisional_state: state.clone(),
            last_rebase: state,
            txx_in_state: Default::default(),
            // seen: LruCache::new(10000),
        }
    }
    /// Creates a State based on the present state of the mempool.
    pub fn to_state(&self) -> State<MeshaCas> {
        self.provisional_state.clone()
    }

    /// Tries to add a transaction to the mempool.
    pub fn apply_transaction(&mut self, tx: &Transaction) -> anyhow::Result<()> {
        if self.provisional_state.transactions.len() < 100 {
            if !self.txx_in_state.insert(tx.hash_nosigs()) {
                return Err(StateError::DuplicateTx.into());
            }
            self.provisional_state.apply_tx(tx)?;
            // self.seen.put(tx.hash_nosigs(), ());
            Ok(())
        } else {
            anyhow::bail!("mempool is full")
        }
    }

    /// Forcibly replaces the internal state of the mempool with the given state.
    pub fn rebase(&mut self, state: State<MeshaCas>) {
        if state.height > self.provisional_state.height {
            log::trace!(
                "rebasing mempool {} => {}",
                self.provisional_state.height,
                state.height
            );
            if !self.provisional_state.transactions.is_empty() {
                let count = self.provisional_state.transactions.len();
                log::warn!("*** THROWING AWAY {} MEMPOOL TXX ***", count);
            }
            assert!(state.transactions.is_empty());
            self.provisional_state = state.clone();
            self.last_rebase = state;
            self.txx_in_state.clear();
        }
    }

    /// Lookups a recent transaction.
    pub fn lookup_recent_tx(&self, hash: TxHash) -> Option<Transaction> {
        None
        // self.seen
        //     .peek(&hash)
        //     .cloned()
        //     .or_else(|| self.provisional_state.transactions.get(&hash).cloned())
    }
}
