use crate::smt::*;
use enum_dispatch::enum_dispatch;
use std::convert::TryInto;

// Internal nodes have 16 children and are identified by their 16-ary hash. Each child is 4 levels closer to the bottom.
// Data nodes represent subtrees that only have one element. They include a bitvec representing remaining steps and the value itself.
#[derive(Clone)]
pub(crate) enum DBNode {
    Internal(InternalNode),
    Data(DataNode),
    Zero,
}
use DBNode::*;

#[enum_dispatch]
pub(crate) trait DBNodeT {
    fn out_ptrs(&self) -> Vec<tmelcrypt::HashVal>;
    fn from_bytes(bts: &[u8]) -> Self;
    fn hash(&self) -> tmelcrypt::HashVal;
}

impl DBNode {
    /// Returns a vector of hash values representing outgoing pointers.
    pub fn out_ptrs(&self) -> Vec<tmelcrypt::HashVal> {
        match self {
            Internal(int) => int.gggc_hashes.to_vec(),
            _ => vec![],
        }
    }

    /// From bytes.
    pub fn from_bytes(bts: &[u8]) -> Self {
        match bts[0] {
            0 => Internal(InternalNode::from_bytes(bts)),
            1 => Data(DataNode::from_bytes(bts)),
            x => panic!("invalid DBNode type {}", x),
        }
    }

    /// To bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Internal(int) => int.to_bytes(),
            Data(dat) => dat.to_bytes(),
            Zero => vec![],
        }
    }

    /// Root-hash.
    pub fn hash(&self) -> tmelcrypt::HashVal {
        match self {
            Internal(int) => int.my_hash,
            Data(dat) => dat.calc_hash(),
            Zero => tmelcrypt::HashVal::default(),
        }
    }
}

fn path_to_idx(path: &[bool]) -> usize {
    let path = &path[..4];
    let mut idx = 0;
    for &p in path {
        if p {
            idx += 1;
        }
        idx <<= 1;
    }
    idx >> 1
}

// Hexary database node. Encoded as 0 || first GGGC || ... || 16th GGGC
#[derive(Clone)]
pub struct InternalNode {
    my_hash: tmelcrypt::HashVal,
    ch_hashes: [tmelcrypt::HashVal; 2],
    gc_hashes: [tmelcrypt::HashVal; 4],
    ggc_hashes: [tmelcrypt::HashVal; 8],
    gggc_hashes: [tmelcrypt::HashVal; 16],
}

fn other(idx: usize) -> usize {
    if idx % 2 == 0 {
        idx + 1
    } else {
        idx - 1
    }
}

impl InternalNode {
    fn from_bytes(bytes: &[u8]) -> Self {
        assert_eq!(bytes[0], 0);
        let bytes = &bytes[1..];
        let zero = tmelcrypt::HashVal::default();
        let mut gggc_hashes = [zero; 16];
        for i in 0..16 {
            gggc_hashes[i] = tmelcrypt::HashVal(bytes[i * 32..i * 32 + 32].try_into().unwrap());
        }
        let mut node = InternalNode {
            my_hash: zero,
            ch_hashes: [zero; 2],
            gc_hashes: [zero; 4],
            ggc_hashes: [zero; 8],
            gggc_hashes,
        };
        node.cache_hashes();
        node
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut toret = Vec::with_capacity(128);
        toret.push(0);
        for v in self.gggc_hashes.iter() {
            toret.extend_from_slice(&v.0);
        }
        toret
    }

    fn cache_hashes(&mut self) {
        if self.my_hash == tmelcrypt::HashVal::default() {
            for i in 0..8 {
                self.ggc_hashes[i] =
                    hash::node(self.gggc_hashes[i * 2], self.gggc_hashes[i * 2 + 1])
            }
            for i in 0..4 {
                self.gc_hashes[i] = hash::node(self.ggc_hashes[i * 2], self.ggc_hashes[i * 2 + 1])
            }
            for i in 0..2 {
                self.ch_hashes[i] = hash::node(self.gc_hashes[i * 2], self.gc_hashes[i * 2 + 1])
            }
            self.my_hash = hash::node(self.ch_hashes[0], self.ch_hashes[1])
        }
    }

    fn get_by_path_rev(&self, path: &[bool], key: tmelcrypt::HashVal, db: ) -> (Vec<u8>, Vec<[u8; 32]>) {

    }
}

/// Subtree with only one element. Encoded as 1 || level || key || value
#[derive(Clone)]
pub struct DataNode {
    level: u8,
    key: tmelcrypt::HashVal,
    data: Vec<u8>,
}

impl DataNode {
    fn from_bytes(bts: &[u8]) -> Self {
        assert_eq!(bts[0], 1);
        let level = bts[1];
        let bytes = &bts[2..];
        DataNode {
            level,
            key: tmelcrypt::HashVal(bytes[..32].try_into().unwrap()),
            data: bytes[32..].to_vec(),
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut toret = Vec::with_capacity(256);
        toret.push(1);
        toret.push(self.level);
        toret.extend_from_slice(&self.key.0);
        toret.extend_from_slice(&self.data);
        toret
    }

    fn calc_hash(&self) -> tmelcrypt::HashVal {
        merk::data_hashes(self.key, &self.data)[self.level as usize]
    }
}
