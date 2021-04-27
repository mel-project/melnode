use blkstructs::{Header, ProposerAction, SealedState};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::BTreeSet;
use tmelcrypt::HashVal;

use crate::traits::DbBackend;

/// A block tree, stored on a particular backend.
pub struct BlockTree<B: DbBackend> {
    inner: Inner<B>,
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
            None => stdcode::deserialize(&self.backend.get(&index_key(blkhash))?).unwrap(),
        };
        let internal = self.backend.get(&main_key(blkhash, height))?;
        Some(stdcode::deserialize(&internal).unwrap())
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
        self.backend.insert(
            &main_key(state.header().hash(), state.inner_ref().height),
            &stdcode::serialize(&InternalValue::from_state(state, action)).unwrap(),
        );
        todo!()
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
