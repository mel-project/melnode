pub fn bts_key(bts: &[u8], key: &[u8]) -> Vec<u8> {
    tmelcrypt::hash_keyed(key, bts).0.to_vec()
}

#[derive(Default)]
pub struct Accumulator {
    buff: Vec<u8>,
    key: Vec<u8>,
}

impl Accumulator {
    pub fn new(key: &[u8]) -> Self {
        Accumulator {
            buff: Vec::new(),
            key: key.to_vec(),
        }
    }
    pub fn add(mut self, bts: &[u8]) -> Self {
        let blen = (bts.len() as u64).to_be_bytes();
        self.buff.extend_from_slice(&blen);
        self.buff.extend_from_slice(bts);
        self
    }
    pub fn hash(self) -> Vec<u8> {
        bts_key(&self.buff, &self.key)
    }
}
