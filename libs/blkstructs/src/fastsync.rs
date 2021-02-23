use autosmt::CompressedProof;
use genawaiter::{sync::GenBoxed, GeneratorState};
use serde::{Deserialize, Serialize};
use tmelcrypt::HashVal;

use crate::{Header, SealedState, SmtMapping, State};

/// A fastsync decoder. The user should call `process_chunk` repeatedly.
pub struct FastSyncDecoder {
    header: Header,
    partial_history: autosmt::Tree,
    partial_coins: autosmt::Tree,
    partial_transactions: autosmt::Tree,
    partial_pools: autosmt::Tree,
    partial_stakes: autosmt::Tree,
}

impl FastSyncDecoder {
    /// Creates a new decoder that validates chunks based on the given header.
    pub fn new(header: Header, dbm: autosmt::DBManager) -> Self {
        let partial_history = dbm.get_tree(Default::default());
        let partial_coins = dbm.get_tree(Default::default());
        let partial_transactions = dbm.get_tree(Default::default());
        let partial_pools = dbm.get_tree(Default::default());
        let partial_stakes = dbm.get_tree(Default::default());
        Self {
            header,
            partial_history,
            partial_coins,
            partial_transactions,
            partial_pools,
            partial_stakes,
        }
    }

    /// Decodes one chunk. Returns Ok(Some(State)) when the decoding process is done, Ok(None) when it is not, and Err(_) when decoding failed.
    pub fn process_chunk(&mut self, chunk: Chunk) -> anyhow::Result<Option<SealedState>> {
        if self.partial_history.root_hash() != self.header.history_hash {
            eprintln!("process history");
            process_tree_chunk(self.header.history_hash, &mut self.partial_history, chunk)?;
        } else if self.partial_coins.root_hash() != self.header.coins_hash {
            eprintln!("process coins");
            process_tree_chunk(self.header.coins_hash, &mut self.partial_coins, chunk)?;
        } else if self.partial_transactions.root_hash() != self.header.transactions_hash {
            eprintln!("process transactions");
            process_tree_chunk(
                self.header.transactions_hash,
                &mut self.partial_transactions,
                chunk,
            )?;
        } else if self.partial_pools.root_hash() != self.header.pools_hash {
            eprintln!("process pools");
            process_tree_chunk(self.header.pools_hash, &mut self.partial_pools, chunk)?;
        } else if self.partial_stakes.root_hash() != self.header.stake_doc_hash {
            eprintln!("process stakes");
            process_tree_chunk(self.header.stake_doc_hash, &mut self.partial_stakes, chunk)?;
        }

        if (self.partial_history.root_hash()) == (self.header.history_hash)
            && (self.partial_coins.root_hash()) == (self.header.coins_hash)
            && (self.partial_transactions.root_hash()) == (self.header.transactions_hash)
            && (self.partial_pools.root_hash()) == (self.header.pools_hash)
            && (self.partial_stakes.root_hash()) == (self.header.stake_doc_hash)
        {
            return Ok(Some(SealedState::force_new(State {
                height: self.header.height,
                history: SmtMapping::new(self.partial_history.clone()),
                coins: SmtMapping::new(self.partial_coins.clone()),
                transactions: SmtMapping::new(self.partial_transactions.clone()),
                fee_pool: self.header.fee_pool,
                fee_multiplier: self.header.fee_multiplier,
                tips: 0,
                dosc_speed: self.header.dosc_speed,
                pools: SmtMapping::new(self.partial_pools.clone()),
                stakes: SmtMapping::new(self.partial_stakes.clone()),
            })));
        }

        Ok(None)
    }
}

fn process_tree_chunk(
    valid_root: HashVal,
    tree: &mut autosmt::Tree,
    chunk: Chunk,
) -> anyhow::Result<()> {
    // validate proof
    let proof = chunk
        .proof
        .decompress()
        .ok_or_else(|| anyhow::anyhow!("could not decompress proof"))?;
    let is_inclusion = proof
        .verify(valid_root, chunk.key_hash, &chunk.data)
        .ok_or_else(|| anyhow::anyhow!("invalid proof"))?;
    if !is_inclusion {
        anyhow::bail!("not a proof of inclusion");
    }
    *tree = tree.set(chunk.key_hash, &chunk.data);
    Ok(())
}

/// A fastsync encoder. Instead of handling I/O inside, the object has a `next_chunk` method to call repeatedly to pull out the encoded stream.
///
/// Because this type also implements `IntoIterator`, a for loop can also be directly used.
pub struct FastSyncEncoder {
    inner: GenBoxed<Chunk>,
}

impl FastSyncEncoder {
    /// Creates a new fastsync encoder based on the given SealedState.
    pub fn new(state: SealedState) -> Self {
        let inner =
            genawaiter::sync::Gen::new_boxed(move |co| async move { fs_encode(co, state).await });
        Self { inner }
    }

    /// Returns the next encoded chunk. Returns None when there isn't any left.
    pub fn next_chunk(&mut self) -> Option<Chunk> {
        match self.inner.resume() {
            GeneratorState::Yielded(val) => Some(val),
            GeneratorState::Complete(_) => None,
        }
    }
}

impl IntoIterator for FastSyncEncoder {
    type Item = Chunk;
    type IntoIter = Box<dyn Iterator<Item = Self::Item>>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.inner.into_iter())
    }
}

async fn fs_encode(co: genawaiter::sync::Co<Chunk>, state: SealedState) {
    // encode the trees
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

async fn fs_encode_tree(co: &genawaiter::sync::Co<Chunk>, tree: &autosmt::Tree) {
    // TODO with library support this can be more efficient
    for (key_hash, data) in tree.iter() {
        let (_, proof) = tree.get(key_hash);
        let proof = proof.compress();
        co.yield_(Chunk {
            key_hash,
            data,
            proof,
        })
        .await;
    }
}

/// A fastsync chunk.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chunk {
    key_hash: HashVal,
    data: Vec<u8>,
    proof: CompressedProof,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::State;
    use autosmt::DBManager;

    #[test]
    fn empty_state() {
        let dbm = DBManager::load(autosmt::MemDB::default());
        let state = State::new_empty(dbm.clone()).seal(None);
        let mut decoder = FastSyncDecoder::new(state.header(), dbm);
        let encoder = FastSyncEncoder::new(state.clone());
        for chunk in encoder {
            if let Some(res) = decoder.process_chunk(chunk).unwrap() {
                assert_eq!(res.header(), state.header());
                return;
            }
        }
        panic!("did not recover a state from the fastsync")
    }
}
