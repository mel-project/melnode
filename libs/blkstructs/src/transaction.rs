use crate::{constants::*, melvm, HexBytes};
use arbitrary::Arbitrary;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::{
    collections::HashMap,
    convert::TryInto,
    fmt::{Display, Formatter},
    num::ParseIntError,
    str::FromStr,
};
use thiserror::Error;
use tmelcrypt::HashVal;

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
    Swap = 0x51,
    LiqDeposit = 0x52,
    LiqWithdraw = 0x53,

    Faucet = 0xff,
}

impl Display for TxKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxKind::Normal => "Normal".fmt(f),
            TxKind::Stake => "Stake".fmt(f),
            TxKind::DoscMint => "DoscMint".fmt(f),
            TxKind::Swap => "Swap".fmt(f),
            TxKind::LiqDeposit => "LiqDeposit".fmt(f),
            TxKind::LiqWithdraw => "LiqWithdraw".fmt(f),
            TxKind::Faucet => "Faucet".fmt(f),
        }
    }
}

/// Transaction represents an individual, serializable Themelio transaction.
#[derive(Clone, Arbitrary, Debug, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct Transaction {
    pub kind: TxKind,
    pub inputs: Vec<CoinID>,
    pub outputs: Vec<CoinData>,
    pub fee: u128,
    pub scripts: Vec<melvm::Covenant>,
    #[serde(with = "stdcode::hex")]
    pub data: Vec<u8>,
    pub sigs: Vec<HexBytes>,
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
        let self_bytes = stdcode::serialize(&s).unwrap();
        tmelcrypt::hash_single(&self_bytes)
    }
    /// sign_ed25519 consumes the transaction, appends an ed25519 signature, adn returns it..
    pub fn signed_ed25519(mut self, sk: tmelcrypt::Ed25519SK) -> Self {
        self.sigs.push(sk.sign(&self.hash_nosigs().0).into());
        self
    }
    /// total_outputs returns a HashMap mapping each type of coin to its total value. Fees will be included in COINTYPE_TMEL.
    pub fn total_outputs(&self) -> HashMap<Denom, u128> {
        let mut toret = HashMap::new();
        for output in self.outputs.iter() {
            let old = *toret.get(&output.denom).unwrap_or(&0);
            toret.insert(output.denom, old + output.value);
        }
        let old = *toret.get(&Denom::Mel).unwrap_or(&0);
        toret.insert(Denom::Mel, old + self.fee);
        toret
    }
    /// scripts_as_map returns a HashMap mapping the hash of each script in the transaction to the script itself.
    pub fn script_as_map(&self) -> HashMap<tmelcrypt::HashVal, melvm::Covenant> {
        let mut toret = HashMap::new();
        for s in self.scripts.iter() {
            toret.insert(s.hash(), s.clone());
        }
        toret
    }

    /// Returns the minimum fee of the transaction at a given fee multiplier, with a given "ballast".
    pub fn base_fee(&self, fee_multiplier: u128, ballast: u128) -> u128 {
        (self.weight().saturating_add(ballast)).saturating_mul(fee_multiplier) >> 16
    }

    /// Returns the weight of the transaction.
    pub fn weight(&self) -> u128 {
        let raw_length = stdcode::serialize(self).unwrap().len() as u128;
        let script_weights: u128 = self
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
        let output_penalty = self.outputs.len() as u128 * 1000;
        let input_boon = self.inputs.len() as u128 * 1000;

        raw_length
            .saturating_add(script_weights)
            .saturating_add(output_penalty)
            .saturating_sub(input_boon)
    }

    /// Convenience function that constructs a CoinID that points to a certain index of this function. Panics if the index is out of bounds.
    pub fn get_coinid(&self, index: u8) -> CoinID {
        assert!((index as usize) < self.outputs.len());
        CoinID {
            txhash: self.hash_nosigs(),
            index,
        }
    }

    /// Convenience function that applies the correct fee.
    /// Call this *before* signing the transaction,
    /// with a ballast that's an upper bound on the number of bytes
    /// added to the transaction as signatures. 100 is a good value for a ballast.
    /// Provide the index of the output to deduct from;
    /// returns None if the output doesn't have enough money to cover fees.
    pub fn applied_fee(
        mut self,
        fee_multiplier: u128,
        ballast: u128,
        deduct_from_idx: usize,
    ) -> Option<Self> {
        let delta_fee = self.base_fee(fee_multiplier, ballast);
        self.fee += delta_fee;
        let deduct_from = self.outputs.get_mut(deduct_from_idx)?;
        deduct_from.value = deduct_from.value.checked_sub(delta_fee)?;
        Some(self)
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

#[derive(Error, Debug, Clone)]
pub enum ParseCoinIDError {
    #[error("could not split into txhash-index")]
    CannotSplit,
    #[error("hex error ({0})")]
    HexError(#[from] hex::FromHexError),
    #[error("parse int error ({0})")]
    ParseIntError(#[from] ParseIntError),
}

impl FromStr for CoinID {
    type Err = ParseCoinIDError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let splitted = s.split('-').collect::<Vec<_>>();
        if splitted.len() != 2 {
            return Err(ParseCoinIDError::CannotSplit);
        }
        let txhash: HashVal = splitted[0].parse()?;
        let index: u8 = splitted[1].parse()?;
        Ok(CoinID { txhash, index })
    }
}

impl Display for CoinID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.txhash.fmt(f)?;
        '-'.fmt(f)?;
        self.index.fmt(f)
    }
}

impl CoinID {
    /// The genesis coin of "zero-zero".
    pub fn zero_zero() -> Self {
        Self {
            txhash: tmelcrypt::HashVal::default(),
            index: 0,
        }
    }

    /// The pseudo-coin-ID for the proposer reward for the given height.
    pub fn proposer_reward(height: u64) -> Self {
        CoinID {
            txhash: tmelcrypt::hash_keyed(b"reward_coin_pseudoid", &height.to_be_bytes()),
            index: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Arbitrary, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
/// The data bound to a coin ID. Contains the "contents" of a coin, i.e. its constraint hash, value, and coin type.
pub struct CoinData {
    pub covhash: tmelcrypt::HashVal,
    pub value: u128,
    // #[serde(with = "stdcode::hex")]
    pub denom: Denom,
    #[serde(with = "stdcode::hex")]
    pub additional_data: Vec<u8>,
}

impl CoinData {
    pub fn additional_data_hex(&self) -> String {
        hex::encode(&self.additional_data)
    }
}

#[derive(Clone, Arbitrary, Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Copy)]
pub enum Denom {
    Mel,
    Sym,
    NomDosc,

    NewCoin,
    Custom(HashVal),
}

impl Denom {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::Mel => b"m".to_vec(),
            Self::Sym => b"s".to_vec(),
            Self::NomDosc => b"d".to_vec(),
            Self::NewCoin => b"".to_vec(),
            Self::Custom(hash) => hash.to_vec(),
        }
    }

    pub fn from_bytes(vec: &[u8]) -> Option<Self> {
        Some(match vec {
            b"m" => Self::Mel,
            b"s" => Self::Sym,
            b"d" => Self::NomDosc,

            b"" => Self::NewCoin,
            other => Self::Custom(HashVal(other.try_into().ok()?)),
        })
    }
}

impl Serialize for Denom {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        DenomInner(self.to_bytes()).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Denom {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let inner = <DenomInner>::deserialize(deserializer)?;
        Denom::from_bytes(&inner.0)
            .ok_or_else(|| serde::de::Error::custom("not the right format for a Denom"))
    }
}

/// A coin denomination, like mel, sym, etc.
#[derive(Serialize, Deserialize, Clone, Arbitrary, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
struct DenomInner(#[serde(with = "stdcode::hex")] Vec<u8>);

#[derive(Serialize, Deserialize, Clone, Arbitrary, Debug, Eq, PartialEq, Hash)]
/// A `CoinData` but coupled with a block height. This is what actually gets stored in the global state, allowing constraints and the validity-checking algorithm to easily access the age of a coin.
pub struct CoinDataHeight {
    pub coin_data: CoinData,
    pub height: u64,
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::{melvm, CoinData, Transaction, MAX_COINVAL};
    use crate::{testing::fixtures::valid_txx, Denom};
    use rstest::*;

    #[rstest]
    fn test_is_well_formed(valid_txx: Vec<Transaction>) {
        for valid_tx in valid_txx.iter() {
            assert!(valid_tx.is_well_formed());
        }
    }

    #[rstest]
    fn test_is_not_well_formed_if_value_gt_max(valid_txx: Vec<Transaction>) {
        // Extract out first coin data from first transaction in valid transactions
        let valid_tx = valid_txx.iter().next().unwrap().clone();
        let valid_outputs = valid_tx.outputs;
        let valid_output = valid_outputs.iter().next().unwrap().clone();

        // Create an invalid tx by setting an invalid output value
        let invalid_output_value = MAX_COINVAL + 1;
        let invalid_output = CoinData {
            value: invalid_output_value,
            ..valid_output
        };
        let invalid_outputs = vec![invalid_output];
        let invalid_tx = Transaction {
            outputs: invalid_outputs,
            ..valid_tx
        };

        // Ensure transaction is not well formed
        assert_eq!(invalid_tx.is_well_formed(), false);
    }

    #[rstest(
        offset => [1 as u128, 2 as u128, 100 as u128]
    )]
    fn test_is_not_well_formed_if_fee_gt_max(offset: u128, valid_txx: Vec<Transaction>) {
        // Extract out first coin data from first transaction in valid transactions
        let valid_tx = valid_txx.iter().next().unwrap().clone();

        // Create an invalid tx by setting an invalid fee value
        let invalid_tx = Transaction {
            fee: MAX_COINVAL + offset,
            ..valid_tx
        };

        // Ensure transaction is not well formed
        assert_eq!(invalid_tx.is_well_formed(), false);
    }

    #[rstest(
        offset => [1, 2, 100]
    )]
    fn test_is_not_well_formed_if_io_gt_max(offset: usize, valid_txx: Vec<Transaction>) {
        // Extract out first coin data from first transaction in valid transactions
        let valid_tx = valid_txx.iter().next().unwrap().clone();
        let valid_outputs = valid_tx.outputs;
        let valid_output = valid_outputs.iter().next().unwrap().clone();

        // Create an invalid tx by setting an invalid output value
        let invalid_output_count = 255 + offset;
        let invalid_outputs = vec![valid_output; invalid_output_count];
        let invalid_tx = Transaction {
            outputs: invalid_outputs,
            ..valid_tx
        };

        // Ensure transaction is not well formed
        assert_eq!(invalid_tx.is_well_formed(), false);

        // TODO: add case for input_count exceeding limit
    }

    #[rstest]
    fn test_hash_no_sigs(valid_txx: Vec<Transaction>) {
        // Check that valid transaction has a non zero number of signatures
        let valid_tx = valid_txx.iter().next().unwrap().clone();
        assert_ne!(valid_tx.sigs.len(), 0);

        // Create a transaction from it which has no signatures
        let mut no_sigs_tx = valid_tx.clone();
        no_sigs_tx.sigs = vec![];

        // Create a transaction from valid which has another signature
        let more_sig_tx = valid_tx.clone();
        let new_sk = tmelcrypt::ed25519_keygen().1;
        let more_sig_tx = more_sig_tx.signed_ed25519(new_sk);

        // Ensure they all hash to same value
        let h1 = valid_tx.hash_nosigs();
        let h2 = no_sigs_tx.hash_nosigs();
        let h3 = more_sig_tx.hash_nosigs();

        assert_eq!(h1, h2);
        assert_eq!(h1, h3);
    }

    #[rstest]
    fn test_sign_sigs(valid_txx: Vec<Transaction>) {
        // Create a transaction from it which has no signatures
        let valid_tx = valid_txx.iter().next().unwrap().clone();
        assert_ne!(valid_tx.sigs.len(), 0);
        let mut no_sigs_tx = valid_tx.clone();
        no_sigs_tx.sigs = vec![];
        assert_eq!(no_sigs_tx.sigs.len(), 0);

        // sign it N times
        let mut mult_signature_tx = no_sigs_tx.clone();
        let n = 5;
        for (_pk, sk) in vec![tmelcrypt::ed25519_keygen(); n].iter() {
            mult_signature_tx = mult_signature_tx.signed_ed25519(*sk);
        }

        // verify it has N signatures
        assert_eq!(mult_signature_tx.sigs.len(), n);

        // sign it M times
        let m = 8;
        for (_pk, sk) in vec![tmelcrypt::ed25519_keygen(); m].iter() {
            mult_signature_tx = mult_signature_tx.signed_ed25519(*sk);
        }

        // verify it has N + M signatures
        assert_eq!(mult_signature_tx.sigs.len(), n + m);
    }

    #[rstest]
    fn test_sign_sigs_and_verify(valid_txx: Vec<Transaction>) {
        // Create a transaction from it which has no signatures
        let valid_tx = valid_txx.iter().next().unwrap().clone();
        assert_ne!(valid_tx.sigs.len(), 0);
        let mut no_sigs_tx = valid_tx.clone();
        no_sigs_tx.sigs = vec![];
        assert_eq!(no_sigs_tx.sigs.len(), 0);

        // create two key pairs
        let (pk1, sk1) = tmelcrypt::ed25519_keygen();
        let (pk2, sk2) = tmelcrypt::ed25519_keygen();

        // sign it
        let mut tx = no_sigs_tx.clone();
        tx = tx.signed_ed25519(sk1);
        tx = tx.signed_ed25519(sk2);

        // verify it is signed by expected keys
        let sig1 = tx.sigs[0].clone();
        let sig2 = tx.sigs[1].clone();

        pk1.verify(&tx.hash_nosigs().to_vec(), &sig1);
        pk2.verify(&tx.hash_nosigs().to_vec(), &sig2);

        assert_eq!(tx.sigs.len(), 2);
    }

    #[rstest]
    fn test_total_output(valid_txx: Vec<Transaction>) {
        // create transaction
        let mut valid_tx = valid_txx.iter().next().unwrap().clone();
        let (pk, _sk) = tmelcrypt::ed25519_keygen();
        let scr = melvm::Covenant::std_ed25519_pk_legacy(pk);

        // insert coins
        let val1 = 100;
        let val2 = 200;
        valid_tx.outputs = vec![
            CoinData {
                covhash: scr.hash(),
                value: val1,
                denom: Denom::NewCoin,
                additional_data: vec![],
            },
            CoinData {
                covhash: scr.hash(),
                value: val2,
                denom: Denom::NewCoin,
                additional_data: vec![],
            },
        ];

        // Check total is valid
        let value_by_coin_type = valid_tx.total_outputs();
        let total: u128 = value_by_coin_type.iter().map(|(_k, v)| *v).sum();

        let fee = 1577000; // Temporary hack
        assert_eq!(total, val1 + val2 + fee);
    }

    #[rstest]
    fn test_script_as_map(valid_txx: Vec<Transaction>) {
        // create transaction
        let valid_tx = valid_txx.iter().next().unwrap().clone();
        let (pk, _sk) = tmelcrypt::ed25519_keygen();
        let _scr = melvm::Covenant::std_ed25519_pk_legacy(pk);

        // add scripts

        // call script_as_map
        let _script_map = valid_tx.script_as_map();

        // verify num scripts = length of returned hashmap

        // verify hashes match expected value
    }

    #[rstest]
    fn test_weight_adjust() {
        // create a transaction

        // call weight with 0 and store

        // call weight with N as adjust and ensure difference is adjust
    }

    #[rstest]
    fn test_weight_does_not_exceed_max_u64() {
        // create a transaction

        // call weight with max u64 size

        // verify result is max u64 size
    }
}
