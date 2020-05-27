use crate::constants::*;
use crate::melscript::*;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use rlp::{Decodable, Encodable};
use rlp_derive::*;
use std::collections::HashMap;
use std::convert::TryFrom;

#[derive(Clone, Copy, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum TxKind {
    Normal = 0x00,
    Stake = 0x10,
    DoscMint = 0x50,
    AuctionBid = 0x51,
    AuctionBuyout = 0x52,
    AuctionFill = 0x53,
}

impl Encodable for TxKind {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        (*self as u8).rlp_append(s)
    }
}

impl Decodable for TxKind {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let raw = u8::decode(rlp)?;
        if let Ok(x) = TxKind::try_from(raw) {
            Ok(x)
        } else {
            Err(rlp::DecoderError::Custom("bad txkind"))
        }
    }
}

/// Transaction represents an individual, RLP-serializable Themelio transaction.
#[derive(RlpEncodable, RlpDecodable, Clone)]
pub struct Transaction {
    pub kind: TxKind,
    pub inputs: Vec<CoinID>,
    pub outputs: Vec<CoinData>,
    pub fee: u64,
    pub scripts: Vec<Script>,
    pub data: Vec<u8>,
    pub sigs: Vec<u8>,
}

impl Transaction {
    /// hash_nosigs returns the hash of the transaction with a zeroed-out signature field. This is what signatures are computed against.
    pub fn hash_nosigs(&self) -> tmelcrypt::HashVal {
        let mut s = self.clone();
        s.sigs = vec![];
        let self_bytes = rlp::encode(&s);
        tmelcrypt::hash_single(&self_bytes)
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
}

#[derive(RlpEncodable, RlpDecodable, Clone)]
pub struct CoinID {
    pub txhash: tmelcrypt::HashVal,
    pub index: u8,
}

#[derive(RlpEncodable, RlpDecodable, Clone)]
pub struct CoinData {
    pub conshash: tmelcrypt::HashVal,
    pub value: u64,
    pub cointype: Vec<u8>,
}
