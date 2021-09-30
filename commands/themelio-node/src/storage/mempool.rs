use std::collections::HashSet;

use lru::LruCache;
use themelio_stf::{State, StateError, Transaction, TxHash};
use tmelcrypt::HashVal;

/// Mempool encapsulates a "mempool" --- a provisional state that is used to form new blocks by stakers, or provisionally validate transactions by auditors.
pub struct Mempool {
    provisional_state: State,
    last_rebase: State,
    txx_in_state: HashSet<TxHash>,
    seen: LruCache<TxHash, Transaction>,
}

impl Mempool {
    /// Create sa new mempool based on a provisional state.
    pub fn new(state: State) -> Self {
        Self {
            provisional_state: state.clone(),
            last_rebase: state,
            txx_in_state: Default::default(),
            seen: LruCache::new(100000),
        }
    }
    /// Creates a State based on the present state of the mempool.
    pub fn to_state(&self) -> State {
        self.provisional_state.clone()
    }

    /// Tries to add a transaction to the mempool.
    pub fn apply_transaction(&mut self, tx: &Transaction) -> Result<(), StateError> {
        // if self.seen.put(tx.hash_nosigs(), tx.clone()).is_some() {
        //     return Err(StateError::DuplicateTx);
        // }
        if !self.txx_in_state.insert(tx.hash_nosigs()) {
            return Err(StateError::DuplicateTx);
        }
        self.provisional_state.apply_tx(tx)?;
        self.seen.put(tx.hash_nosigs(), tx.clone());
        Ok(())
    }

    /// Forcibly replaces the internal state of the mempool with the given state.
    pub fn rebase(&mut self, state: State) {
        if state.height > self.provisional_state.height {
            #[cfg(not(feature = "metrics"))]
            log::trace!(
                "rebasing mempool {} => {}",
                self.provisional_state.height,
                state.height
            );
            #[cfg(feature = "metrics")]
            log::trace!(
                "hostname={} public_ip={} rebasing mempool {} => {}",
                crate::prometheus::HOSTNAME.as_str(), crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
                self.provisional_state.height,
                state.height
            );
            if self.provisional_state.transactions.root_hash() != HashVal::default() {
                let count = self.provisional_state.transactions.val_iter().count();
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
        self.seen
            .peek(&hash)
            .cloned()
            .or_else(|| self.provisional_state.transactions.get(&hash).0)
    }
}
