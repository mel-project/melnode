use crate::{constants::*, melscript};
use arbitrary::Arbitrary;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::collections::HashMap;

#[derive(
    Clone,
    Copy,
    IntoPrimitive,
    TryFromPrimitive,
    Eq,
    PartialEq,
    Arbitrary,
    Debug,
    Serialize_repr,
    Deserialize_repr,
    Hash,
)]
#[repr(u8)]
/// An enumeration of all the different possible transaction kinds. Currently contains a "faucet" kind that will be (obviously) removed in production.
pub enum TxKind {
    Normal = 0x00,
    Stake = 0x10,
    DoscMint = 0x50,
    AuctionBid = 0x51,
    AuctionBuyout = 0x52,
    AuctionFill = 0x53,

    Faucet = 0xff,
}

/// Transaction represents an individual, serializable Themelio transaction.
#[derive(Clone, Arbitrary, Debug, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct Transaction {
    pub kind: TxKind,
    pub inputs: Vec<CoinID>,
    pub outputs: Vec<CoinData>,
    pub fee: u64,
    pub scripts: Vec<melscript::Script>,
    pub data: Vec<u8>,
    pub sigs: Vec<Vec<u8>>,
}

impl Transaction {
    pub fn empty_test() -> Self {
        Transaction {
            kind: TxKind::Normal,
            inputs: Vec::new(),
            outputs: Vec::new(),
            fee: 0,
            scripts: Vec::new(),
            data: Vec::new(),
            sigs: Vec::new(),
        }
    }
    /// checks whether or not the transaction is well formed, respecting coin size bounds and such.
    pub fn is_well_formed(&self) -> bool {
        // check bounds
        for out in self.outputs.iter() {
            if out.value > MAX_COINVAL {
                return false;
            }
        }
        if self.fee > MAX_COINVAL {
            return false;
        }
        if self.outputs.len() > 255 || self.inputs.len() > 255 {
            return false;
        }
        true
    }
    /// hash_nosigs returns the hash of the transaction with a zeroed-out signature field. This is what signatures are computed against.
    pub fn hash_nosigs(&self) -> tmelcrypt::HashVal {
        let mut s = self.clone();
        s.sigs = vec![];
        let self_bytes = bincode::serialize(&s).unwrap();
        tmelcrypt::hash_single(&self_bytes)
    }
    /// sign_ed25519 appends an ed25519 signature to the transaction.
    pub fn sign_ed25519(mut self, sk: tmelcrypt::Ed25519SK) -> Self {
        self.sigs.push(sk.sign(&self.hash_nosigs().0));
        self
    }
    /// total_outputs returns a HashMap mapping each type of coin to its total value. Fees will be included in COINTYPE_TMEL.
    pub fn total_outputs(&self) -> HashMap<Vec<u8>, u64> {
        let mut toret = HashMap::new();
        for output in self.outputs.iter() {
            let old = *toret.get(&output.cointype).unwrap_or(&0);
            toret.insert(output.cointype.clone(), old + output.value);
        }
        let old = *toret.get(COINTYPE_TMEL).unwrap_or(&0);
        toret.insert(COINTYPE_TMEL.to_vec(), old + self.fee);
        toret
    }
    /// scripts_as_map returns a HashMap mapping the hash of each script in the transaction to the script itself.
    pub fn script_as_map(&self) -> HashMap<tmelcrypt::HashVal, melscript::Script> {
        let mut toret = HashMap::new();
        for s in self.scripts.iter() {
            toret.insert(s.hash(), s.clone());
        }
        toret
    }
    /// Returns the weight of the transaction. Takes in an adjustment factor that should be a generous estimate of signature size.
    pub fn weight(&self, adjust: u64) -> u64 {
        let raw_length = bincode::serialize(self).unwrap().len() as u64 + adjust;
        let script_weights: u64 = self
            .scripts
            .iter()
            .map(|scr| scr.weight().unwrap_or_default())
            .sum();
        // we price in the net state "burden".
        // how much is that? let's assume that history is stored for 1 month. this means that "stored" bytes are around 240 times more expensive than "temporary" bytes.
        // we also take into account that stored stuff is probably going to be stuffed into something much cheaper (e.g. HDD rather than RAM), almost certainly more than 24 times cheaper.
        // so it's probably "safe-ish" to say that stored things are 10 times more expensive than temporary things.
        // econ efficiency/market stability wise it's probably okay to overprice storage, but probably not okay to underprice it.
        // blockchain-spamming-as-HDD arbitrage is going to be really bad for the blockchain.
        // penalize 1000 for every output and boost 1000 for every input. "non-refundable" because the fee can't be subzero
        let output_penalty = self.outputs.len() as u64 * 1000;
        let input_boon = self.inputs.len() as u64 * 1000;

        raw_length
            .saturating_add(script_weights)
            .saturating_add(output_penalty)
            .saturating_sub(input_boon)
    }
}

#[derive(
    Serialize, Deserialize, Clone, Debug, Copy, Arbitrary, Ord, PartialOrd, Eq, PartialEq, Hash,
)]
/// A coin ID, consisting of a transaction hash and index. Uniquely identifies a coin in Themelio's history.
pub struct CoinID {
    pub txhash: tmelcrypt::HashVal,
    pub index: u8,
}

impl CoinID {
    /// The genesis coin of "zero-zero".
    pub fn zero_zero() -> Self {
        Self {
            txhash: tmelcrypt::HashVal::default(),
            index: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Arbitrary, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
/// The data bound to a coin ID. Contains the "contents" of a coin, i.e. its constraint hash, value, and coin type.
pub struct CoinData {
    pub conshash: tmelcrypt::HashVal,
    pub value: u64,
    pub cointype: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Arbitrary, Debug)]
/// A `CoinData` but coupled with a block height. This is what actually gets stored in the global state, allowing constraints and the validity-checking algorithm to easily access the age of a coin.
pub struct CoinDataHeight {
    pub coin_data: CoinData,
    pub height: u64,
}
