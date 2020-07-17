use crate::constants::*;
use crate::transaction as txn;
use im::HashMap;
use parking_lot::RwLock;
use rayon::prelude::*;
use rlp_derive::*;
use std::convert::TryInto;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Instant;
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
    #[error("invalid sequential proof of work")]
    InvalidMelPoW,
    #[error("auction bid at wrong time")]
    BidWrongTime,
}

/// World state of the Themelio blockchain
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct State {
    pub height: u64,
    pub history: SmtMapping<u64, Header>,
    pub coins: SmtMapping<txn::CoinID, txn::CoinDataHeight>,
    pub transactions: SmtMapping<HashVal, txn::Transaction>,

    pub fee_pool: u64,
    pub fee_multiplier: u64,

    pub dosc_multiplier: u64,
    pub auction_bids: SmtMapping<HashVal, txn::Transaction>,
    pub met_price: u64,
    pub mel_price: u64,

    pub stake_doc: SmtMapping<txn::CoinID, Vec<u8>>,
}

impl State {
    /// Generates a test genesis state, with a given starting coin.
    pub fn test_genesis(
        db: autosmt::DBManager,
        start_micromels: u64,
        start_conshash: tmelcrypt::HashVal,
    ) -> Self {
        assert!(start_micromels <= MAX_COINVAL);
        let mut empty = State::new_empty(db);
        // insert coin out of nowhere
        let init_coin = txn::CoinData {
            conshash: start_conshash,
            value: start_micromels,
            cointype: COINTYPE_TMEL.to_vec(),
        };
        empty.coins.insert(
            txn::CoinID {
                txhash: tmelcrypt::HashVal([0; 32]),
                index: 0,
            },
            txn::CoinDataHeight {
                coin_data: init_coin,
                height: 0,
            },
        );
        empty
    }
    /// Applies a single transaction.
    pub fn apply_tx(&mut self, tx: &txn::Transaction) -> Result<(), TxApplicationError> {
        self.apply_tx_batch(std::slice::from_ref(tx))
    }

    /// Applies a batch of transactions. The order of the transactions in txx do not matter.
    pub fn apply_tx_batch(&mut self, txx: &[txn::Transaction]) -> Result<(), TxApplicationError> {
        // clone self first
        let mut newself = self.clone();
        // first ensure that all the transactions are well-formed
        for tx in txx {
            if !tx.is_well_formed() {
                return Err(TxApplicationError::MalformedTx);
            }
            newself.transactions.insert(tx.hash_nosigs(), tx.clone());
        }
        let lnewself = RwLock::new(newself);
        let start_time = Instant::now();
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
        // then we apply the nondefault checks in parallel
        let res: Result<Vec<()>, TxApplicationError> = txx
            .par_iter()
            .filter(|tx| tx.kind != txn::TxKind::Normal)
            .map(|tx| State::apply_tx_special(&lnewself, tx))
            .collect();
        res?;
        // we commit the changes
        //panic!("COMMIT?!");
        *self = lnewself.read().clone();
        Ok(())
    }

    /// Finalizes a state into a block. This consumes the state.
    pub fn finalize(mut self) -> FinalizedState {
        // synthesize auction fill as needed
        if self.height % AUCTION_INTERVAL == 0 && !self.auction_bids.is_empty() {
            self.synthesize_afill()
        }
        // TODO stake stuff
        // create the finalized state
        FinalizedState(self)
    }

    // ----------- helpers start here ------------

    fn new_empty(db: autosmt::DBManager) -> Self {
        let empty_tree = db.get_tree(tmelcrypt::HashVal::default());
        State {
            height: 0,
            history: SmtMapping::new(empty_tree.clone()),
            coins: SmtMapping::new(empty_tree.clone()),
            transactions: SmtMapping::new(empty_tree.clone()),
            fee_pool: 1000000,
            fee_multiplier: 1000,
            dosc_multiplier: 1,
            auction_bids: SmtMapping::new(empty_tree.clone()),
            met_price: MICRO_CONVERTER,
            mel_price: MICRO_CONVERTER,
            stake_doc: SmtMapping::new(empty_tree),
        }
    }

    fn synthesize_afill(&mut self) {
        todo!("synthesize afill")
    }

    // apply inputs
    fn apply_tx_inputs(
        lself: &RwLock<Self>,
        tx: &txn::Transaction,
    ) -> Result<(), TxApplicationError> {
        let scripts = tx.script_as_map();
        // build a map of input coins
        let mut in_coins: HashMap<Vec<u8>, u64> = HashMap::new();
        // iterate through the inputs
        for coin_id in tx.inputs.iter() {
            let (coin_data, _) = lself.read().coins.get(coin_id);
            match coin_data {
                None => return Err(TxApplicationError::NonexistentCoin(*coin_id)),
                Some(coin_data) => {
                    log::trace!(
                        "coin_data {:?} => {:?} for txid {:?}",
                        coin_id,
                        coin_data,
                        tx.hash_nosigs()
                    );
                    let script = scripts.get(&coin_data.coin_data.conshash).ok_or(
                        TxApplicationError::NonexistentScript(coin_data.coin_data.conshash),
                    )?;
                    // we skip checking the script if it's ABID and the tx type is buyout or fill
                    if !(coin_data.coin_data.conshash == tmelcrypt::hash_keyed(b"ABID", b"special")
                        && (tx.kind == txn::TxKind::AuctionBuyout
                            || tx.kind == txn::TxKind::AuctionFill))
                        && !script.check(tx)
                    {
                        return Err(TxApplicationError::ViolatesScript(
                            coin_data.coin_data.conshash,
                        ));
                    }
                    // spend the coin by deleting
                    lself.write().coins.delete(coin_id);
                    in_coins.insert(
                        coin_data.coin_data.cointype.clone(),
                        in_coins.get(&coin_data.coin_data.cointype).unwrap_or(&0)
                            + coin_data.coin_data.value,
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
    // apply outputs
    fn apply_tx_outputs(
        lself: &RwLock<Self>,
        tx: &txn::Transaction,
    ) -> Result<(), TxApplicationError> {
        for (index, coin_data) in tx.outputs.iter().enumerate() {
            let height = lself.read().height;
            // if conshash is zero, this destroys the coins permanently
            if coin_data.conshash.0 != [0; 32] {
                lself.write().coins.insert(
                    txn::CoinID {
                        txhash: tx.hash_nosigs(),
                        index: index.try_into().unwrap(),
                    },
                    txn::CoinDataHeight {
                        coin_data: coin_data.clone(),
                        height,
                    },
                );
            }
        }
        Ok(())
    }
    // apply special effects
    fn apply_tx_special(
        lself: &RwLock<Self>,
        tx: &txn::Transaction,
    ) -> Result<(), TxApplicationError> {
        match tx.kind {
            txn::TxKind::DoscMint => State::apply_tx_special_doscmint(lself, tx),
            txn::TxKind::AuctionBid => State::apply_tx_special_auctionbid(lself, tx),
            txn::TxKind::AuctionBuyout => State::apply_tx_special_auctionbuyout(lself, tx),
            txn::TxKind::AuctionFill => {
                panic!("auction fill transaction processed in normal pipeline")
            }
            txn::TxKind::Stake => unimplemented!("stake"),
            txn::TxKind::Normal => {
                panic!("tried to apply special effects of a non-special transaction")
            }
        }
    }
    // dosc minting
    fn apply_tx_special_doscmint(
        lself: &RwLock<Self>,
        tx: &txn::Transaction,
    ) -> Result<(), TxApplicationError> {
        let lself = lself.read();
        // construct puzzle seed
        let chi = tmelcrypt::hash_single(&rlp::encode(
            tx.inputs.get(0).ok_or(TxApplicationError::MalformedTx)?,
        ));
        // compute difficulty
        let new_dosc = *tx
            .total_outputs()
            .get(&cointype_dosc(lself.height))
            .ok_or(TxApplicationError::MalformedTx)?;
        let raw_difficulty = new_dosc * lself.dosc_multiplier;
        let true_difficulty = 64 - raw_difficulty.leading_zeros() as usize;
        // check the proof
        let mp_proof =
            melpow::Proof::from_bytes(&tx.data).ok_or(TxApplicationError::MalformedTx)?;
        if !mp_proof.verify(&chi.0, true_difficulty) {
            Err(TxApplicationError::InvalidMelPoW)
        } else {
            Ok(())
        }
    }
    // auction bidding
    fn apply_tx_special_auctionbid(
        lself: &RwLock<Self>,
        tx: &txn::Transaction,
    ) -> Result<(), TxApplicationError> {
        let mut lself = lself.write();
        // must be in first half of auction
        if lself.height % 20 >= 10 {
            return Err(TxApplicationError::BidWrongTime);
        }
        // data must be a 32-byte conshash
        if tx.data.len() != 32 {
            return Err(TxApplicationError::MalformedTx);
        }
        // first output stores the price bid for the mets
        let first_output = tx.outputs.get(0).ok_or(TxApplicationError::MalformedTx)?;
        if first_output.cointype != cointype_dosc(lself.height) {
            return Err(TxApplicationError::MalformedTx);
        }
        // first output must have an empty script
        if first_output.conshash != tmelcrypt::hash_keyed(b"ABID", b"special") {
            return Err(TxApplicationError::MalformedTx);
        }
        // save transaction to auction list
        lself.auction_bids.insert(tx.hash_nosigs(), tx.clone());
        Ok(())
    }
    // auction buyout
    fn apply_tx_special_auctionbuyout(
        lself: &RwLock<Self>,
        tx: &txn::Transaction,
    ) -> Result<(), TxApplicationError> {
        let mut lself = lself.write();
        // find the one and only ABID input
        let abid_txx: Vec<txn::Transaction> = tx
            .inputs
            .iter()
            .filter_map(|cid| lself.auction_bids.get(&cid.txhash).0)
            .collect();
        if abid_txx.len() != 1 {
            return Err(TxApplicationError::MalformedTx);
        }
        let abid_txx = &abid_txx[0];
        // validate that the first output fills the order
        let first_output: &txn::CoinData =
            tx.outputs.get(0).ok_or(TxApplicationError::MalformedTx)?;
        if first_output.cointype != COINTYPE_TMET
            || first_output.value < abid_txx.outputs[0].value
            || first_output.conshash.0.to_vec() != abid_txx.data
        {
            return Err(TxApplicationError::MalformedTx);
        }
        // remove the order from the order book
        lself.auction_bids.delete(&abid_txx.hash_nosigs());
        Ok(())
    }
}

/// FinalizedState represents an immutable state at a finalized block height. It cannot be constructed except through finalizing a State or restoring from persistent storage.
pub struct FinalizedState(State);

impl FinalizedState {
    /// Returns a reference to the State finalized within.
    pub fn inner_ref(&self) -> &State {
        &self.0
    }
    /// Returns the block header represented by the finalized state.
    pub fn header(&self) -> Header {
        let inner = &self.0;
        // panic!()
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
    /// Creates a new unfinalized state representing the next block.
    pub fn next_state(&self) -> State {
        let mut new = self.0.clone();
        // advance the numbers
        new.history.insert(self.0.height, self.header());
        new.height += 1;
        new
    }
}

#[derive(RlpEncodable, RlpDecodable, Copy, Clone, Debug)]
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

impl Header {
    pub fn hash(&self) -> tmelcrypt::HashVal {
        tmelcrypt::hash_single(&rlp::encode(self))
    }
}

/// SmtMapping is a type-safe, constant-time clonable, imperative-style interface to a sparse Merkle tree.
pub struct SmtMapping<K: rlp::Encodable, V: rlp::Decodable + rlp::Encodable> {
    pub mapping: autosmt::Tree,
    _phantom_k: PhantomData<K>,
    _phantom_v: PhantomData<V>,
}

impl<K: rlp::Encodable, V: rlp::Decodable + rlp::Encodable> Debug for SmtMapping<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.mapping.root_hash().fmt(f)
    }
}

impl<K: rlp::Encodable, V: rlp::Decodable + rlp::Encodable> Clone for SmtMapping<K, V> {
    fn clone(&self) -> Self {
        SmtMapping::new(self.mapping.clone())
    }
}

impl<K: rlp::Encodable, V: rlp::Decodable + rlp::Encodable> SmtMapping<K, V> {
    /// Returns true iff the mapping is empty.
    pub fn is_empty(&self) -> bool {
        self.root_hash().0 == [0; 32]
    }
    /// new converts a type-unsafe SMT to a SmtMapping
    pub fn new(tree: autosmt::Tree) -> Self {
        SmtMapping {
            mapping: tree,
            _phantom_k: PhantomData,
            _phantom_v: PhantomData,
        }
    }
    /// get obtains a mapping
    pub fn get(&self, key: &K) -> (Option<V>, autosmt::FullProof) {
        let key = tmelcrypt::hash_single(&rlp::encode(key));
        let (v_bytes, proof) = self.mapping.get(key);
        match v_bytes.len() {
            0 => (None, proof),
            _ => {
                let res: V = rlp::decode(&v_bytes).expect("SmtMapping saw invalid data");
                (Some(res), proof)
            }
        }
    }
    /// insert inserts a mapping, replacing any existing mapping
    pub fn insert(&mut self, key: K, val: V) {
        let key = tmelcrypt::hash_single(&rlp::encode(&key));
        let newmap = self.mapping.set(key, &rlp::encode(&val));
        // eprintln!(
        //     "{:?} ==insert=> {:?}",
        //     self.mapping.root_hash(),
        //     newmap.root_hash()
        // );
        self.mapping = newmap
    }
    /// delete deletes a mapping, replacing the mapping with a mapping to the empty bytestring
    pub fn delete(&mut self, key: &K) {
        let key = tmelcrypt::hash_single(&rlp::encode(key));
        let newmap = self.mapping.set(key, b"");
        self.mapping = newmap
    }
    /// root_hash returns the root hash
    pub fn root_hash(&self) -> HashVal {
        self.mapping.root_hash()
    }
}
