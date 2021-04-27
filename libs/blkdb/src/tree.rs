use blkstructs::{Header, ProposerAction, SealedState};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tmelcrypt::HashVal;

use crate::traits::DbBackend;

/// A block tree, stored on a particular backend.
pub struct BlockTree<B: DbBackend> {
    inner: RwLock<Inner<B>>,
}

impl<B: DbBackend> BlockTree<B> {
    /// Create a new BlockTree.
    pub fn new(backend: B) -> Self {
        let inner = RwLock::new(Inner { backend });
        Self { inner }
    }
}

/// Lower-level helper struct that provides fail-safe basic operations.
struct Inner<B: DbBackend> {
    backend: B,
}

impl<B: DbBackend> Inner<B> {
    /// Gets a block from the database.
    fn get_block(&self, blkhash: HashVal, height: Option<u64>) -> Option<InternalValue> {
        let height = match height {
            Some(height) => height,
            None => self.index_get(blkhash)?,
        };
        self.internal_get(blkhash, height)
    }

    /// Inserts a block into the database
    fn insert_block(
        &mut self,
        state: SealedState,
        action: Option<ProposerAction>,
    ) -> Option<InternalValue> {
        if let Some(val) = self.get_block(state.header().hash(), Some(state.inner_ref().height)) {
            return Some(val);
        }
        // we carefully insert the block to avoid inconsistency:
        // - first we insert the block itself
        // - then we point the parent to it
        // - then we insert into the blkhash index
        // - then we update the tips list
        let header = state.header();
        let blkhash = header.hash();
        // insert the block
        self.internal_insert(
            blkhash,
            header.height,
            InternalValue::from_state(state, action),
        );
        // insert into parent
        if let Some(mut parent) =
            self.get_block(header.previous, Some(header.height.saturating_sub(1)))
        {
            parent.next.insert(blkhash);
            self.internal_insert(header.previous, parent.header.height, parent);
        }
        // insert into blkhash index
        self.index_insert(blkhash, header.height);
        // update tips list
        self.tip_remove(header.previous);
        self.tip_insert(blkhash, header.height);
        None
    }

    fn internal_insert(&mut self, blkhash: HashVal, height: u64, value: InternalValue) {
        self.backend.insert(
            &main_key(blkhash, height),
            &stdcode::serialize(&value).unwrap(),
        );
    }

    fn index_insert(&mut self, blkhash: HashVal, height: u64) {
        self.backend
            .insert(&index_key(blkhash), &stdcode::serialize(&height).unwrap());
    }

    fn tip_insert(&mut self, blkhash: HashVal, height: u64) {
        self.backend
            .insert(&tip_key(blkhash), &stdcode::serialize(&height).unwrap());
    }

    fn internal_get(&self, blkhash: HashVal, height: u64) -> Option<InternalValue> {
        stdcode::deserialize(&self.backend.get(&main_key(blkhash, height))?)
            .expect("cannot deserialize internal value")
    }

    fn index_get(&self, blkhash: HashVal) -> Option<u64> {
        stdcode::deserialize(&self.backend.get(&index_key(blkhash))?)
            .expect("cannot deserialize index value")
    }

    fn tip_remove(&self, blkhash: HashVal) {
        self.backend.remove(&tip_key(blkhash));
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct InternalValue {
    header: Header,
    partial_state: Vec<u8>,
    action: Option<ProposerAction>,
    next: BTreeSet<HashVal>,
}

impl InternalValue {
    fn from_state(state: SealedState, action: Option<ProposerAction>) -> Self {
        Self {
            header: state.header(),
            partial_state: state.partial_encoding(),
            action,
            next: Default::default(),
        }
    }
}

fn main_key(blkhash: HashVal, height: u64) -> [u8; 40] {
    let mut toret = [0u8; 40];
    toret[..8].copy_from_slice(&height.to_be_bytes());
    toret[8..].copy_from_slice(&blkhash);
    toret
}

fn tip_key(blkhash: HashVal) -> [u8; 40] {
    main_key(blkhash, u64::MAX - 1)
}

fn index_key(blkhash: HashVal) -> [u8; 40] {
    main_key(blkhash, u64::MAX)
}
