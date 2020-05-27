use rlp::{Decodable, Encodable};
use std::convert::TryInto;

#[derive(Copy, Clone)]
pub struct HashVal(pub [u8; 32]);

impl Encodable for HashVal {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        let arr = self.0.as_ref();
        arr.rlp_append(s)
    }
}

impl Decodable for HashVal {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let v = Vec::decode(r)?;
        if v.len() != 32 {
            Err(rlp::DecoderError::Custom("HashVal not 32 bytes"))
        } else {
            let v = v.as_slice();
            let v = v.try_into().unwrap();
            Ok(HashVal(v))
        }
    }
}

pub fn hash_single(val: &[u8]) -> HashVal {
    let b3h = blake3::hash(val);
    HashVal((*b3h.as_bytes().as_ref()).try_into().unwrap())
}
