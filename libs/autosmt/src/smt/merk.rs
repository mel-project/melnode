use crate::smt::*;
use bitvec::prelude::*;
use std::fmt::Debug;

pub fn key_to_path(key: tmelcrypt::HashVal) -> [bool; 256] {
    let mut toret = [false; 256];
    // enumerate each byte
    for (i, k_i) in key.0.iter().enumerate() {
        // walk through the bits
        for j in 0..8 {
            toret[i * 8 + j] = k_i & (0b1000_0000 >> j) != 0;
        }
    }
    toret
}

pub fn data_hashes(key: tmelcrypt::HashVal, data: &[u8]) -> Vec<tmelcrypt::HashVal> {
    let path = merk::key_to_path(key);
    let mut ptr = hash::datablock(data);
    let mut hashes = Vec::new();
    hashes.push(ptr);
    for data_on_right in path.iter().rev() {
        if *data_on_right {
            // add the opposite hash
            ptr = hash::node(tmelcrypt::HashVal::default(), ptr);
        } else {
            ptr = hash::node(ptr, tmelcrypt::HashVal::default());
        }
        hashes.push(ptr)
    }
    hashes.reverse();
    hashes
}

#[derive(Debug, Clone)]
pub struct FullProof(pub Vec<tmelcrypt::HashVal>);

impl FullProof {
    pub fn compress(&self) -> CompressedProof {
        let FullProof(proof_nodes) = self;
        assert_eq!(proof_nodes.len(), 256);
        // build bitmap
        let mut bitmap = bitvec![Msb0, u8; 0; 256];
        for (i, pn) in proof_nodes.iter().enumerate() {
            if *pn == tmelcrypt::HashVal::default() {
                bitmap.set(i, true);
            }
        }
        let mut bitmap_slice = bitmap.as_slice().to_vec();
        for pn in proof_nodes.iter() {
            if *pn != tmelcrypt::HashVal::default() {
                bitmap_slice.extend_from_slice(&pn.0.to_vec());
            }
        }
        CompressedProof(bitmap_slice)
    }
}

impl std::fmt::Display for FullProof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hexa: Vec<String> = self.0.iter().map(|x| hex::encode(x.0)).collect();
        hexa.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct CompressedProof(pub Vec<u8>);

impl std::fmt::Display for CompressedProof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str: String = hex::encode(&self.0);
        std::fmt::Display::fmt(&str, f)
    }
}
