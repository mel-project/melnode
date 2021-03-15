mod sled_tree;
use std::sync::Arc;

use blkstructs::{
    ConsensusProof, GenesisConfig, SealedState, State, StateError, Transaction,
};
use dashmap::DashMap;
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

    highest_height: SledMap<u8, u64>,
    history: SledMap<u64, Vec<u8>>,
    proofs: SledMap<u64, ConsensusProof>,
    history_cache: DashMap<u64, SealedState>,
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
        let highest_height = SledMap::new(db.open_tree("height").unwrap());
        let history: SledMap<u64, Vec<u8>> = SledMap::new(db.open_tree("history").unwrap());
        let proofs = SledMap::new(db.open_tree("proofs").unwrap());
        let history_cache = DashMap::new();
        let forest = autosmt::Forest::load(SledTreeDB::new(db.open_tree("autosmt").unwrap()));

        // initialize stuff
        if history.get(&0).is_none() {
            history.insert(
                0,
                State::genesis(&forest, genesis)
                    .seal(None)
                    .partial_encoding(),
            );
            highest_height.insert(0, 0);
        }

        let mempool_state = SealedState::from_partial_encoding_infallible(
            &history.get(&highest_height.get(&0).unwrap()).unwrap(),
            &forest,
        )
        .next_state();
        Self {
            mempool: Mempool {
                provisional_state: mempool_state,
            },
            highest_height,
            history,
            proofs,
            history_cache,
            forest,
        }
    }

    /// Obtain the highest state.
    pub fn highest_state(&self) -> SealedState {
        self.get_state(self.highest_height()).unwrap()
    }

    /// Obtain the highest height.
    pub fn highest_height(&self) -> u64 {
        self.highest_height.get(&0).unwrap_or_default()
    }

    /// Obtain a historical SealedState.
    pub fn get_state(&self, height: u64) -> Option<SealedState> {
        if let Some(val) = self.history_cache.get(&height) {
            Some(val.clone())
        } else {
            let raw_value = self.history.get(&height)?;
            let state = SealedState::from_partial_encoding_infallible(&raw_value, &self.forest);
            self.history_cache.insert(height, state.clone());
            Some(state)
        }
    }

    /// Obtain a historical ConsensusProof.
    pub fn get_consensus(&self, height: u64) -> Option<ConsensusProof> {
        self.proofs.get(&height)
    }

    /// Consumes a block, applying it to the current state.
    pub fn apply_block(
        &mut self,
        blk: blkstructs::Block,
        cproof: ConsensusProof,
    ) -> anyhow::Result<()> {
        let highest_height = self.highest_height();
        let mut last_state = self
            .get_state(highest_height)
            .expect("database corruption: no state at the stated highest height")
            .next_state();
        last_state.apply_tx_batch(&blk.transactions.iter().cloned().collect::<Vec<_>>())?;
        let new_state = last_state.seal(blk.proposer_action);
        if new_state.header() != blk.header {
            anyhow::bail!(
                "header mismatch! got {:#?}, expected {:#?}, after applying block {:#?}",
                new_state.header(),
                blk.header,
                blk
            );
        }
        let new_height = new_state.inner_ref().height;
        self.history
            .insert(new_height, new_state.partial_encoding());
        // TODO: check consensus proof
        self.proofs.insert(new_height, cproof);

        // save highest height last to prevent inconsistencies
        self.highest_height.insert(0, new_height);

        log::debug!(
            "block {}, txcount={}, hash={:?} APPLIED",
            new_height,
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

/// Mempool encapsulates a "mempool" --- a provisional state that is used to form new blocks by stakers, or provisionally validate transactions by auditors.
pub struct Mempool {
    provisional_state: State,
    // TODO: caches if benchmarks prove them helpful
}

impl neosymph::TxLookup for Mempool {
    fn lookup(&self, hash: HashVal) -> Option<Transaction> {
        self.provisional_state.transactions.get(&hash).0
    }
}

impl Mempool {
    /// Creates a State based on the present state of the mempool.
    pub fn to_state(&self) -> State {
        self.provisional_state.clone()
    }

    /// Tries to add a transaction to the mempool.
    pub fn apply_transaction(&mut self, tx: &Transaction) -> Result<(), StateError> {
        self.provisional_state.apply_tx(tx)
    }

    /// Forcibly replaces the internal state of the mempool with the given state, returning the previous state.
    pub fn rebase(&mut self, state: State) -> State {
        std::mem::replace(&mut self.provisional_state, state)
    }
}
