use genawaiter::{sync::GenBoxed, GeneratorState};
use serde::Serialize;

use crate::SealedState;

/// A fastsync encoder. Instead of handling I/O inside, the user should call `next_chunk` repeatedly to pull out the encoded stream.
///
/// Because this type also implements `IntoIterator`, a for loop can also be directly used.
pub struct FastSyncEncoder {
    inner: GenBoxed<Vec<u8>>,
}

impl FastSyncEncoder {
    /// Creates a new fastsync encoder based on the given SealedState.
    pub fn new(state: SealedState) -> Self {
        let inner =
            genawaiter::sync::Gen::new_boxed(move |co| async move { fs_encode(co, state).await });
        Self { inner }
    }

    /// Returns the next encoded chunk. Returns None when there isn't any left.
    pub fn next_chunk(&mut self) -> Option<Vec<u8>> {
        match self.inner.resume() {
            GeneratorState::Yielded(val) => Some(val),
            GeneratorState::Complete(_) => None,
        }
    }
}

impl IntoIterator for FastSyncEncoder {
    type Item = Vec<u8>;
    type IntoIter = Box<dyn Iterator<Item = Self::Item>>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.inner.into_iter())
    }
}

async fn fs_encode(co: genawaiter::sync::Co<Vec<u8>>, state: SealedState) {
    // first encode header
    fs_encode_prim(&co, &state.header()).await;
    // then encode the trees
    let state = state.inner_ref();
    for tree in &[
        &state.history.mapping,
        &state.coins.mapping,
        &state.transactions.mapping,
        &state.pools.mapping,
        &state.stakes.mapping,
    ] {
        fs_encode_tree(&co, tree).await;
    }
}

async fn fs_encode_prim(co: &genawaiter::sync::Co<Vec<u8>>, item: &impl Serialize) {
    let serialized = stdcode::serialize(item).expect("fastsync cannot encode primitive");
    co.yield_(serialized).await;
}

async fn fs_encode_tree(co: &genawaiter::sync::Co<Vec<u8>>, tree: &autosmt::Tree) {
    // TODO with library support this can be more efficient
    for (k_hash, elem) in tree.iter() {
        let (_, proof) = tree.get(k_hash);
        let to_yield = stdcode::serialize(&(k_hash, elem, proof.compress()))
            .expect("fastsync cannot encode tree element");
        co.yield_(to_yield).await;
    }
}
