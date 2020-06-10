use crate::transaction as txn;
use rayon::prelude::*;
use rlp_derive::*;
use std::collections::HashMap;
use std::convert::TryInto;
use std::marker::PhantomData;
use std::sync::RwLock;
use thiserror::Error;
use tmelcrypt::HashVal;

#[derive(Error, Debug)]
/// A error that happens while applying a transaction to a state
pub enum TxApplicationError {
    #[error("malformed transaction")]
    MalformedTx,
    #[error("attempted to spend non-existent coin {:?}", .0)]
    NonexistentCoin(txn::CoinID),
    #[error("unbalanced inputs and outputs")]
    UnbalancedInOut,
    #[error("referenced non-existent script {:?}", .0)]
    NonexistentScript(tmelcrypt::HashVal),
    #[error("does not satisfy script {:?}", .0)]
    ViolatesScript(tmelcrypt::HashVal),
}

/// World state of the Themelio blockchain
#[non_exhaustive]
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

impl<T: autosmt::Database> Clone for State<T> {
    fn clone(&self) -> Self {
        State {
            height: self.height,
            history: self.history.clone(),
            coins: self.coins.clone(),
            transactions: self.transactions.clone(),
            fee_pool: self.fee_pool,
            fee_multiplier: self.fee_multiplier,
            dosc_multiplier: self.dosc_multiplier,
            auction_bids: self.auction_bids.clone(),
            met_price: self.met_price,
            mel_price: self.mel_price,
            stake_doc: self.stake_doc.clone(),
        }
    }
}

impl<T: autosmt::Database> State<T> {
    /// applies a single transaction.
    pub fn apply_tx(&mut self, tx: &txn::Transaction) -> Result<(), TxApplicationError> {
        self.apply_tx_batch(std::slice::from_ref(tx))
    }
    /// applies a batch of transactions. The order of the transactions in txx do not matter.
    pub fn apply_tx_batch(&mut self, txx: &[txn::Transaction]) -> Result<(), TxApplicationError> {
        // clone self first
        let newself = self.clone();
        // first ensure that all the transactions are well-formed
        for tx in txx {
            if !tx.is_well_formed() {
                return Err(TxApplicationError::MalformedTx);
            }
        }
        let lnewself = RwLock::new(newself);
        // then we apply the outputs in parallel
        let res: Result<Vec<()>, TxApplicationError> = txx
            .par_iter()
            .map(|tx| State::apply_tx_outputs(&lnewself, tx))
            .collect();
        res?;
        // then we apply the inputs in parallel
        let res: Result<Vec<()>, TxApplicationError> = txx
            .par_iter()
            .map(|tx| State::apply_tx_inputs(&lnewself, tx))
            .collect();
        res?;
        // we commit the changes
        *self = lnewself.read().unwrap().clone();
        Ok(())
    }
    fn apply_tx_fees(
        lself: &RwLock<Self>,
        tx: &txn::Transaction,
    ) -> Result<(), TxApplicationError> {
        println!("skipping application of fees");
        Ok(())
    }
    fn apply_tx_inputs(
        lself: &RwLock<Self>,
        tx: &txn::Transaction,
    ) -> Result<(), TxApplicationError> {
        let scripts = tx.script_as_map();
        // build a map of input coins
        let mut in_coins: HashMap<Vec<u8>, u64> = HashMap::new();
        // iterate through the inputs
        for coin_id in tx.inputs.iter() {
            let (coin_data, _) = lself.read().unwrap().coins.get(coin_id);
            match coin_data {
                None => return Err(TxApplicationError::NonexistentCoin(*coin_id)),
                Some(coin_data) => {
                    let script = scripts
                        .get(&coin_data.conshash)
                        .ok_or(TxApplicationError::NonexistentScript(coin_data.conshash))?;
                    if !script.check(tx) {
                        return Err(TxApplicationError::ViolatesScript(coin_data.conshash));
                    }
                    // spend the coin by deleting
                    lself.write().unwrap().coins.delete(coin_id);
                    in_coins.insert(
                        coin_data.cointype.clone(),
                        in_coins.get(&coin_data.cointype).unwrap_or(&0) + coin_data.value,
                    );
                }
            }
        }
        // balance inputs and outputs. ignore outputs with empty cointype (they create a new token kind)
        let out_coins = tx.total_outputs();
        if tx.kind != txn::TxKind::DoscMint {
            for (currency, value) in out_coins.iter() {
                if !currency.is_empty() && *value != *in_coins.get(currency).unwrap_or(&u64::MAX) {
                    return Err(TxApplicationError::UnbalancedInOut);
                }
            }
        }
        Ok(())
    }
    fn apply_tx_outputs(
        lself: &RwLock<Self>,
        tx: &txn::Transaction,
    ) -> Result<(), TxApplicationError> {
        for (index, coin_data) in tx.outputs.iter().enumerate() {
            lself.write().unwrap().coins.insert(
                &txn::CoinID {
                    txhash: tx.hash_nosigs(),
                    index: index.try_into().unwrap(),
                },
                coin_data,
            );
        }
        Ok(())
    }
}

/// FinalizedState represents an immutable state at a finalized block height. It cannot be constructed except through finalizing a State or restoring from persistent storage.
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
