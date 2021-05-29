#![allow(clippy::upper_case_acronyms)]

mod sled_tree;
use std::{collections::HashSet, sync::Arc};

use blkdb::{traits::DbBackend, BlockTree};
use themelio_stf::{ConsensusProof, GenesisConfig, SealedState, State, StateError, Transaction};
use lru::LruCache;
use parking_lot::RwLock;
pub use sled_tree::*;
use tmelcrypt::HashVal;

/// An alias for a shared NodeStorage.
pub type SharedStorage = Arc<RwLock<NodeStorage>>;

/// NodeStorage encapsulates all storage used by a Themelio full node (auditor or staker).
pub struct NodeStorage {
    mempool: Mempool,

    history: BlockTree<BoringDbBackend>,
    forest: novasmt::Forest,
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
    pub fn new(db: boringdb::Database, genesis: GenesisConfig) -> Self {
        // Identify the genesis by the genesis ID
        let genesis_id = tmelcrypt::hash_single(stdcode::serialize(&genesis).unwrap());
        let dict = db.open_dict(&format!("genesis{}", genesis_id)).unwrap();
        let forest = novasmt::Forest::new(BoringDbSmt::new(dict.clone()));
        let mut history = BlockTree::new(BoringDbBackend { dict }, forest.clone(), true);

        // initialize stuff
        if history.get_tips().is_empty() {
            history.set_genesis(State::genesis(&forest, genesis).seal(None), &[]);
        }

        let mempool_state = history.get_tips()[0].to_state().next_state();
        Self {
            mempool: Mempool {
                provisional_state: mempool_state.clone(),
                last_rebase: mempool_state,
                txx_in_state: HashSet::new(),
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
        let tips = self.history.get_tips();
        if tips.len() != 1 {
            log::error!(
                "multiple tips: {:#?}",
                tips.iter().map(|v| v.header()).collect::<Vec<_>>()
            );
        }
        tips.into_iter().map(|v| v.header().height).max().unwrap()
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
        blk: themelio_stf::Block,
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
            "block {}, txcount={}, hash={:?} APPLIED (highest height {})",
            highest_height + 1,
            blk.transactions.len(),
            blk.header.hash(),
            self.highest_height(),
        );
        let next = self.highest_state().next_state();
        self.mempool_mut().rebase(next);
        Ok(())
    }

    /// Convenience method to "share" storage.
    pub fn share(self) -> SharedStorage {
        Arc::new(RwLock::new(self))
    }

    /// Gets the forest.
    pub fn forest(&self) -> novasmt::Forest {
        self.forest.clone()
    }
}

struct BoringDbBackend {
    dict: boringdb::Dict,
}

impl DbBackend for BoringDbBackend {
    fn insert(&mut self, key: &[u8], value: &[u8]) -> Option<Vec<u8>> {
        self.dict
            .insert(key.to_vec(), value.to_vec())
            .unwrap()
            .map(|v| v.to_vec())
    }

    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.dict.get(key).unwrap().map(|v| v.to_vec())
    }

    fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        self.dict.remove(key).unwrap().map(|v| v.to_vec())
    }

    fn key_range(&self, start: &[u8], end: &[u8]) -> Vec<Vec<u8>> {
        self.dict
            .range(start..=end)
            .unwrap()
            .map(|v| v.unwrap().0.to_vec())
            .collect()
    }
}

/// Mempool encapsulates a "mempool" --- a provisional state that is used to form new blocks by stakers, or provisionally validate transactions by auditors.
pub struct Mempool {
    provisional_state: State,
    last_rebase: State,
    txx_in_state: HashSet<HashVal>,
    seen: LruCache<HashVal, Transaction>, // TODO: caches if benchmarks prove them helpful
}

impl Mempool {
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
            log::debug!(
                "rebasing mempool {} => {}",
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

    pub fn lookup(&self, hash: HashVal) -> Option<Transaction> {
        self.seen
            .peek(&hash)
            .cloned()
            .or_else(|| self.provisional_state.transactions.get(&hash).0)
    }
}
