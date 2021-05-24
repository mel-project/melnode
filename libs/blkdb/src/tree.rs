use crate::traits::DbBackend;
use blkstructs::{Block, Header, ProposerAction, SealedState};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::{
    collections::{BTreeSet, HashSet},
    convert::TryInto,
};
use thiserror::Error;
use tmelcrypt::HashVal;

/// A block tree, stored on a particular backend.
pub struct BlockTree<B: DbBackend> {
    inner: Inner<B>,
    forest: novasmt::Forest,
    canonical: bool,
}

impl<B: DbBackend> BlockTree<B> {
    /// Create a new BlockTree.
    pub fn new(backend: B, forest: novasmt::Forest, canonical: bool) -> Self {
        let inner = Inner {
            backend,
            canonical,
            cache: Default::default(),
        };
        Self {
            inner,
            forest,
            canonical,
        }
    }

    /// Attempts to apply a block.
    pub fn apply_block(
        &mut self,
        block: &Block,
        init_metadata: &[u8],
    ) -> Result<(), ApplyBlockErr> {
        let previous = self
            .inner
            .get_block(
                block.header.previous,
                Some(block.header.height.saturating_sub(1)),
            )
            .ok_or(ApplyBlockErr::ParentNotFound(block.header.previous))?;
        let previous = previous.to_state(&self.forest, &self.inner.cache);
        let next_state = previous
            .apply_block(block)
            .map_err(ApplyBlockErr::CannotValidate)?;

        // apply block should already have checked this
        assert_eq!(next_state.header(), block.header);

        self.inner.insert_block(next_state, init_metadata);
        Ok(())
    }

    /// Get all the cursors at a given height.
    pub fn get_at_height(&self, height: u64) -> Vec<Cursor<'_, B>> {
        self.inner
            .all_at_height(height)
            .into_iter()
            .map(|v| {
                self.get_cursor(v)
                    .expect("did not get expected block at height")
            })
            .collect()
    }

    /// Obtains a *cursor* pointing to a particular block in the tree. The cursor has a lifetime bound that prevents the blocktree from mutating when cursors exist.
    pub fn get_cursor(&self, hash: HashVal) -> Option<Cursor<'_, B>> {
        let internal = self.inner.get_block(hash, None)?;
        Some(Cursor {
            tree: self,
            internal,
        })
    }

    /// Obtains a *mutable cursor* pointing to a particular block in the tree. The cursor has a lifetime bound that prevents the blocktree from mutating when cursors exist.
    pub fn get_cursor_mut(&mut self, hash: HashVal) -> Option<CursorMut<'_, B>> {
        let internal = self.inner.get_block(hash, None)?;
        Some(CursorMut {
            tree: self,
            internal,
        })
    }

    /// Get a vector of "tips" in the blockchain
    pub fn get_tips(&self) -> Vec<Cursor<'_, B>> {
        let tip_keys = self.inner.all_tips();
        tip_keys
            .into_iter()
            .filter_map(|v| self.get_cursor(v))
            .collect()
    }

    /// Sets the genesis block of the tree. This also prunes all elements that do not belong to the given genesis block.
    pub fn set_genesis(&mut self, state: SealedState, init_metadata: &[u8]) {
        let state_hash = state.header().hash();
        if self.get_cursor(state_hash).is_none() {
            // directly insert the block now
            assert!(self.inner.insert_block(state, init_metadata).is_none());
        }

        let old_genesis = self.get_tips().into_iter().next().map(|v| {
            let mut v = v;
            while let Some(parent) = v.parent() {
                v = parent;
            }
            v
        });

        // remove all non-descendants
        let mut descendants = HashSet::new();
        {
            let mut stack: Vec<Cursor<_>> = vec![self
                .get_cursor(state_hash)
                .expect("just-set genesis is gone?!")];
            while let Some(top) = stack.pop() {
                descendants.insert(top.header().hash());
                for child in top.children() {
                    stack.push(child);
                }
            }
        }
        let mut todel = HashSet::new();
        if let Some(old_genesis) = old_genesis {
            if old_genesis.header().hash() != state_hash {
                // use this cursor to traverse
                let mut stack: Vec<Cursor<_>> = vec![old_genesis];
                while let Some(top) = stack.pop() {
                    if !descendants.contains(&top.header().hash()) {
                        // this is a damned one!
                        todel.insert(top.header());
                        for child in top.children() {
                            stack.push(child)
                        }
                    }
                }
            }
        }
        // okay now we go through the whole todel sequence
        let mut todel = todel.into_iter().collect::<Vec<_>>();
        todel.sort_unstable_by_key(|v| v.height);
        for todel in todel {
            self.inner.remove_orphan(todel.hash(), Some(todel.height));
            if self.canonical {
                // we also delete all the SMTs
                self.forest.delete_tree(todel.coins_hash.0);
                self.forest.delete_tree(todel.pools_hash.0);
                self.forest.delete_tree(todel.stakes_hash.0);
                self.forest.delete_tree(todel.transactions_hash.0);
                self.forest.delete_tree(todel.history_hash.0);
            }
        }
    }

    /// Creates a GraphViz string that represents all the blocks in the tree.
    pub fn debug_graphviz(&self, visitor: impl Fn(&Cursor<'_, B>) -> String) -> String {
        let mut stack = self.get_tips();
        let tips = self
            .get_tips()
            .iter()
            .map(|v| v.header())
            .collect::<HashSet<_>>();
        let mut seen = HashSet::new();
        let mut output = String::new();
        writeln!(&mut output, "digraph G {{").unwrap();
        while let Some(top) = stack.pop() {
            if seen.insert(top.header()) {
                if tips.contains(&top.header()) {
                    writeln!(
                        &mut output,
                        "\"{}\" [label={}, shape=rectangle, style=filled, fillcolor=red];",
                        top.header().hash(),
                        top.header().height
                    )
                    .unwrap();
                } else {
                    writeln!(
                        &mut output,
                        "\"{}\" [label={}, shape=rectangle, style=filled, fillcolor=\"{}\"];",
                        top.header().hash(),
                        top.header().height,
                        visitor(&top),
                    )
                    .unwrap();
                }
                if let Some(parent) = top.parent() {
                    writeln!(
                        &mut output,
                        "\"{}\" -> \"{}\";",
                        top.header().hash(),
                        top.header().previous
                    )
                    .unwrap();
                    stack.push(parent);
                }
            }
        }
        writeln!(&mut output, "}}").unwrap();
        output
    }
}

/// A cursor, pointing to something inside the block tree.
pub struct Cursor<'a, B: DbBackend> {
    tree: &'a BlockTree<B>,
    internal: InternalValue,
}

impl<'a, B: DbBackend> Clone for Cursor<'a, B> {
    fn clone(&self) -> Self {
        Self {
            tree: self.tree,
            internal: self.internal.clone(),
        }
    }
}

impl<'a, B: DbBackend> Cursor<'a, B> {
    /// Converts to a SealedState.
    pub fn to_state(&self) -> SealedState {
        self.internal
            .to_state(&self.tree.forest, &self.tree.inner.cache)
    }

    /// Extracts the header.
    pub fn header(&self) -> Header {
        self.internal.header
    }

    /// Extracts the metadata.
    pub fn metadata(&self) -> &[u8] {
        &self.internal.metadata
    }

    /// Returns a vector of child cursors.
    pub fn children(&self) -> Vec<Self> {
        self.internal
            .next
            .iter()
            .map(|hash| self.tree.get_cursor(*hash).expect("dangling child pointer"))
            .collect()
    }

    /// Returns the parent of this block.
    pub fn parent(&self) -> Option<Self> {
        self.tree.get_cursor(self.internal.header.previous)
    }
}

/// A mutable cursor, pointing to something inside the block tree.
pub struct CursorMut<'a, B: DbBackend> {
    tree: &'a mut BlockTree<B>,
    internal: InternalValue,
}

impl<'a, B: DbBackend> CursorMut<'a, B> {
    /// Converts to a SealedState.
    pub fn to_state(&self) -> SealedState {
        self.internal
            .to_state(&self.tree.forest, &self.tree.inner.cache)
    }

    /// Extracts the header.
    pub fn header(&self) -> Header {
        self.internal.header
    }

    /// Extracts the metadata.
    pub fn metadata(&self) -> &[u8] {
        &self.internal.metadata
    }

    /// Sets the metadata.
    pub fn set_metadata(&mut self, metadata: &[u8]) {
        self.internal.metadata = metadata.to_vec();
        self.tree.inner.internal_insert(
            self.header().hash(),
            self.header().height,
            self.internal.clone(),
        );
    }

    /// Consumes and returns the parent of this block.
    pub fn parent(self) -> Option<Self> {
        self.tree.get_cursor_mut(self.internal.header.previous)
    }

    /// "Downgrades" the cursor to an immutable cursor.
    pub fn downgrade(self) -> Cursor<'a, B> {
        Cursor {
            tree: self.tree,
            internal: self.internal,
        }
    }
}

/// An error returned when applying a block
#[derive(Error, Debug)]
pub enum ApplyBlockErr {
    #[error("parent `{0}` not found")]
    ParentNotFound(HashVal),
    #[error("validation error: `{0}`")]
    CannotValidate(blkstructs::StateError),
    #[error("header mismatch")]
    HeaderMismatch,
}

/// Lower-level helper struct that provides fail-safe basic operations.
struct Inner<B: DbBackend> {
    backend: B,
    canonical: bool,
    // cached SealedStates for non-canonical mode. this is required so that inserted blocks are persistent.
    cache: DashMap<HashVal, SealedState>,
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

    /// Removes a block with no parent.
    fn remove_orphan(&mut self, blkhash: HashVal, height: Option<u64>) {
        let current = self
            .get_block(blkhash, height)
            .expect("trying to remove nonexistent orphan");
        debug_assert!(self.get_block(current.header.previous, None).is_none());
        // remove from tips, index, then main
        self.tip_remove(blkhash);
        self.index_remove(blkhash);
        self.internal_remove(blkhash, current.header.height);
        // finally delete from cache
        self.cache.remove(&blkhash);
    }

    /// Inserts a block into the database
    fn insert_block(
        &mut self,
        mut state: SealedState,
        init_metadata: &[u8],
    ) -> Option<InternalValue> {
        // if let Some(val) = self.get_block(state.header().hash(), Some(state.inner_ref().height)) {
        //     return Some(val);
        // }
        let action = state.proposer_action().cloned();
        // we carefully insert the block to avoid inconsistency:
        // - first we insert the block itself
        // - then we point the parent to it
        // - then we insert into the blkhash index
        // - then we update the tips list
        let header = state.header();
        let blkhash = header.hash();
        // stabilize the block onto disk
        if self.canonical {
            state.save_smts();
        }
        // insert the block
        self.internal_insert(
            blkhash,
            header.height,
            InternalValue::from_state(&state, action, init_metadata.to_vec()),
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
        // update cache
        self.cache.insert(state.header().hash(), state);
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
        Some(
            stdcode::deserialize(&self.backend.get(&main_key(blkhash, height))?)
                .expect("cannot deserialize internal value"),
        )
    }

    fn internal_remove(&mut self, blkhash: HashVal, height: u64) {
        self.backend.remove(&main_key(blkhash, height));
    }

    fn index_get(&self, blkhash: HashVal) -> Option<u64> {
        Some(
            stdcode::deserialize(&self.backend.get(&index_key(blkhash))?)
                .expect("cannot deserialize index value"),
        )
    }

    fn index_remove(&mut self, blkhash: HashVal) {
        self.backend.remove(&index_key(blkhash));
    }

    fn tip_remove(&mut self, blkhash: HashVal) {
        self.backend.remove(&tip_key(blkhash));
    }

    fn all_tips(&self) -> Vec<HashVal> {
        let raw = self
            .backend
            .key_range(&tip_key(HashVal([0x00; 32])), &tip_key(HashVal([0xff; 32])));
        raw.into_iter()
            .map(|v| HashVal((&v[8..]).try_into().expect("corrupt tip key")))
            .collect()
    }

    fn all_at_height(&self, height: u64) -> Vec<HashVal> {
        let raw = self.backend.key_range(
            &main_key(HashVal([0x00; 32]), height),
            &main_key(HashVal([0xff; 32]), height),
        );
        raw.into_iter()
            .map(|v| HashVal((&v[8..]).try_into().expect("corrupt tip key")))
            .collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct InternalValue {
    header: Header,
    partial_state: Vec<u8>,
    action: Option<ProposerAction>,
    next: BTreeSet<HashVal>,
    metadata: Vec<u8>,
}

impl InternalValue {
    fn from_state(state: &SealedState, action: Option<ProposerAction>, metadata: Vec<u8>) -> Self {
        Self {
            header: state.header(),
            partial_state: state.partial_encoding(),
            action,
            next: Default::default(),
            metadata,
        }
    }

    fn to_state(
        &self,
        forest: &novasmt::Forest,
        cache: &DashMap<HashVal, SealedState>,
    ) -> SealedState {
        cache
            .entry(self.header.hash())
            .or_insert_with(|| {
                SealedState::from_partial_encoding_infallible(&self.partial_state, forest)
            })
            .value()
            .clone()
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
