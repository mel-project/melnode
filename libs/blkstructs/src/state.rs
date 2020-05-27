use crate::transaction as txn;
use rlp_derive::*;
use std::marker::PhantomData;
use tmelcrypt::HashVal;

/// State represents the world state of the Themelio blockchain. It intentionally doesn't implement Clone.
pub struct State<T: autosmt::Database> {
    pub height: u64,
    pub history: SmtMapping<u64, Header, T>,
    pub coins: SmtMapping<txn::CoinID, txn::CoinData, T>,
    pub transactions: SmtMapping<HashVal, txn::Transaction, T>,

    pub fee_pool: u64,
    pub fee_multiplier: u64,

    pub dosc_multiplier: u64,
    pub auction_bids: SmtMapping<HashVal, txn::Transaction, T>,
    pub met_price: u64,
    pub mel_price: u64,

    pub stake_doc: SmtMapping<txn::CoinID, Vec<u8>, T>,
}

/// FinalizedState represents an immutable state at a finalized block height. It cannot be constructed except through finalizing a State.
pub struct FinalizedState<T: autosmt::Database>(State<T>);

impl<T: autosmt::Database> FinalizedState<T> {
    /// inner_ref returns a reference to the State finalized within.
    pub fn inner_ref(&self) -> &State<T> {
        &self.0
    }
    /// header returns the block header represented by the finalized state.
    pub fn header(&self) -> Header {
        let inner = &self.0;
        Header {
            height: inner.height,
            history_hash: inner.history.root_hash(),
            coins_hash: inner.coins.root_hash(),
            transactions_hash: inner.transactions.root_hash(),
            fee_pool: inner.fee_pool,
            fee_multiplier: inner.fee_multiplier,
            dosc_multiplier: inner.dosc_multiplier,
            auction_bids_hash: inner.auction_bids.root_hash(),
            met_price: inner.met_price,
            mel_price: inner.mel_price,
            stake_doc_hash: inner.stake_doc.root_hash(),
        }
    }
}

#[derive(RlpEncodable, RlpDecodable, Copy, Clone)]
pub struct Header {
    pub height: u64,
    pub history_hash: HashVal,
    pub coins_hash: HashVal,
    pub transactions_hash: HashVal,
    pub fee_pool: u64,
    pub fee_multiplier: u64,
    pub dosc_multiplier: u64,
    pub auction_bids_hash: HashVal,
    pub met_price: u64,
    pub mel_price: u64,
    pub stake_doc_hash: HashVal,
}

/// SmtMapping is a type-safe, constant-time clonable, imperative-style interface to a sparse Merkle tree.
pub struct SmtMapping<K: rlp::Encodable, V: rlp::Decodable + rlp::Encodable, D: autosmt::Database> {
    pub mapping: autosmt::Tree<D>,
    _phantom_k: PhantomData<K>,
    _phantom_v: PhantomData<V>,
}

impl<K: rlp::Encodable, V: rlp::Decodable + rlp::Encodable, D: autosmt::Database> Clone
    for SmtMapping<K, V, D>
{
    fn clone(&self) -> Self {
        SmtMapping::new(&self.mapping)
    }
}

impl<K: rlp::Encodable, V: rlp::Decodable + rlp::Encodable, D: autosmt::Database>
    SmtMapping<K, V, D>
{
    /// new converts a type-unsafe SMT to a SmtMapping
    pub fn new(tree: &autosmt::Tree<D>) -> Self {
        let tree = tree.clone();
        SmtMapping {
            mapping: tree,
            _phantom_k: PhantomData,
            _phantom_v: PhantomData,
        }
    }
    /// get obtains a mapping
    pub fn get(&self, key: &K) -> (Option<V>, autosmt::FullProof) {
        let key = autosmt::hash::index(&rlp::encode(key));
        let (v_bytes, proof) = self.mapping.get(key);
        match v_bytes {
            Some(v_bytes) => {
                let res: V = rlp::decode(&v_bytes).expect("SmtMapping saw invalid data");
                (Some(res), proof)
            }
            None => (None, proof),
        }
    }
    /// insert inserts a mapping, replacing any existing mapping
    pub fn insert(&mut self, key: &K, val: &V) {
        let key = autosmt::hash::index(&rlp::encode(key));
        let newmap = self.mapping.set(key, &rlp::encode(val));
        self.mapping = newmap
    }
    /// delete deletes a mapping, replacing the mapping with a mapping to the empty bytestring
    pub fn delete(&mut self, key: &K) {
        let key = autosmt::hash::index(&rlp::encode(key));
        let newmap = self.mapping.set(key, b"");
        self.mapping = newmap
    }
    /// root_hash returns the root hash
    pub fn root_hash(&self) -> HashVal {
        HashVal(self.mapping.root_hash())
    }
}
