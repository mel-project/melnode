#![allow(clippy::upper_case_acronyms)]

mod sled_tree;
use std::sync::Arc;

use blkdb::{backends::InMemoryBackend, traits::DbBackend, BlockTree};
use blkstructs::{ConsensusProof, GenesisConfig, SealedState, State, StateError, Transaction};
use lru::LruCache;
use parking_lot::RwLock;
pub use sled_tree::*;
mod sled_map;
pub use sled_map::*;
use tmelcrypt::HashVal;

/// An alias for a shared NodeStorage.
pub type SharedStorage = Arc<RwLock<NodeStorage>>;

/// NodeStorage encapsulates all storage used by a Themelio full node (auditor or staker).
pub struct NodeStorage {
    mempool: Mempool,

    history: BlockTree<SledBackend>,
    forest: autosmt::Forest,
}

impl NodeStorage {
    /// Gets an immutable reference to the mempool.
    pub fn mempool(&self) -> &Mempool {
        &self.mempool
    }

    /// Gets a mutable reference to the mempool.
    pub fn mempool_mut(&mut self) -> &mut Mempool {
        &mut self.mempool
    }

    /// Opens a NodeStorage, given a sled database.
    pub fn new(db: sled::Db, genesis: GenesisConfig) -> Self {
        let forest = autosmt::Forest::load(SledTreeDB::new(db.open_tree("autosmt").unwrap()));
        let blktree_backend = SledBackend {
            inner: db.open_tree("node_blktree").unwrap(),
        };
        let mut history = BlockTree::new(blktree_backend, forest.clone());

        // initialize stuff
        if history.get_tips().is_empty() {
            history.set_genesis(State::genesis(&forest, genesis).seal(None), &[]);
        }

        let mempool_state = history.get_tips()[0].to_state().next_state();
        Self {
            mempool: Mempool {
                provisional_state: mempool_state,
                seen: LruCache::new(100000),
            },
            history,
            forest,
        }
    }

    /// Obtain the highest state.
    pub fn highest_state(&self) -> SealedState {
        self.get_state(self.highest_height()).unwrap()
    }

    /// Obtain the highest height.
    pub fn highest_height(&self) -> u64 {
        self.history.get_tips()[0].header().height
    }

    /// Obtain a historical SealedState.
    pub fn get_state(&self, height: u64) -> Option<SealedState> {
        self.history
            .get_at_height(height)
            .get(0)
            .map(|v| v.to_state())
    }

    /// Obtain a historical ConsensusProof.
    pub fn get_consensus(&self, height: u64) -> Option<ConsensusProof> {
        let height = self
            .history
            .get_at_height(height)
            .into_iter()
            .next()
            .unwrap();
        Some(stdcode::deserialize(height.metadata()).unwrap())
    }

    /// Consumes a block, applying it to the current state.
    pub fn apply_block(
        &mut self,
        blk: blkstructs::Block,
        cproof: ConsensusProof,
    ) -> anyhow::Result<()> {
        let highest_height = self.highest_height();
        if blk.header.height != highest_height + 1 {
            anyhow::bail!(
                "cannot apply block {} to height {}",
                blk.header.height,
                highest_height
            );
        }

        self.history
            .apply_block(&blk, &stdcode::serialize(&cproof).unwrap())?;

        log::debug!(
            "block {}, txcount={}, hash={:?} APPLIED",
            highest_height + 1,
            blk.transactions.len(),
            blk.header.hash()
        );
        Ok(())
    }

    /// Convenience method to "share" storage.
    pub fn share(self) -> SharedStorage {
        Arc::new(RwLock::new(self))
    }
}

struct SledBackend {
    inner: sled::Tree,
}

impl DbBackend for SledBackend {
    fn insert(&mut self, key: &[u8], value: &[u8]) -> Option<Vec<u8>> {
        self.inner.insert(key, value).unwrap().map(|v| v.to_vec())
    }

    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.inner.get(key).unwrap().map(|v| v.to_vec())
    }

    fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        self.inner.remove(key).unwrap().map(|v| v.to_vec())
    }

    fn key_range(&self, start: &[u8], end: &[u8]) -> Vec<Vec<u8>> {
        self.inner
            .range(start..=end)
            .map(|v| v.unwrap().0.to_vec())
            .collect()
    }
}

/// Mempool encapsulates a "mempool" --- a provisional state that is used to form new blocks by stakers, or provisionally validate transactions by auditors.
pub struct Mempool {
    provisional_state: State,
    seen: LruCache<HashVal, Transaction>, // TODO: caches if benchmarks prove them helpful
}

impl neosymph::TxLookup for Mempool {
    fn lookup(&self, hash: HashVal) -> Option<Transaction> {
        self.seen
            .peek(&hash)
            .cloned()
            .or_else(|| self.provisional_state.transactions.get(&hash).0)
    }
}

impl Mempool {
    /// Creates a State based on the present state of the mempool.
    pub fn to_state(&self) -> State {
        self.provisional_state.clone()
    }

    /// Tries to add a transaction to the mempool.
    pub fn apply_transaction(&mut self, tx: &Transaction) -> Result<(), StateError> {
        if self.seen.put(tx.hash_nosigs(), tx.clone()).is_some() {
            return Err(StateError::DuplicateTx);
        }
        self.provisional_state.apply_tx(tx)
    }

    /// Forcibly replaces the internal state of the mempool with the given state, returning the previous state.
    pub fn rebase(&mut self, state: State) -> State {
        std::mem::replace(&mut self.provisional_state, state)
    }
}
