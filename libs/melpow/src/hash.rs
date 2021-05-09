use smallvec::SmallVec;

use crate::node::SVec;

pub fn bts_key(bts: &[u8], key: &[u8]) -> SVec<u8> {
    SVec::from_slice(&tmelcrypt::hash_keyed(key, bts))
}

#[derive(Default)]
pub struct Accumulator {
    buff: SmallVec<[u8; 512]>,
    key: Vec<u8>,
}

impl Accumulator {
    pub fn new(key: &[u8]) -> Self {
        Accumulator {
            buff: SmallVec::new(),
            key: key.to_vec(),
        }
    }

    #[inline]
    pub fn add(&mut self, bts: &[u8]) -> &mut Self {
        let blen = (bts.len() as u64).to_be_bytes();
        self.buff.extend_from_slice(&blen);
        self.buff.extend_from_slice(bts);
        self
    }

    #[inline]
    pub fn hash(&self) -> SVec<u8> {
        bts_key(&self.buff, &self.key)
    }
}
