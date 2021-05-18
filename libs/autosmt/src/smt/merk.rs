use crate::smt::*;
use bitvec::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::io::Read;

pub(crate) const MSB_SET: u8 = 0b1000_0000;

pub fn key_to_path(key: tmelcrypt::HashVal) -> [bool; 256] {
    let mut toret = [false; 256];
    // enumerate each byte
    for (i, k_i) in key.0.iter().enumerate() {
        // walk through the bits
        for j in 0..8 {
            toret[i * 8 + j] = k_i & (MSB_SET >> j) != 0;
        }
    }
    toret
}

pub fn data_hashes(key: tmelcrypt::HashVal, data: &[u8]) -> Vec<tmelcrypt::HashVal> {
    let path = merk::key_to_path(key);
    let mut ptr = hash::datablock(data);
    let mut hashes = vec![ptr];

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

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
/// A full proof with 256 levels.
pub struct FullProof(pub Vec<tmelcrypt::HashVal>);

impl FullProof {
    /// Compresses the proof to a serializable form.
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

    /// Verifies that this merkle branch is a valid proof of inclusion or non-inclusion. To check proofs of non-inclusion, set val to the empty vector.
    pub fn verify(&self, root: tmelcrypt::HashVal, key: tmelcrypt::HashVal, val: &[u8]) -> bool {
        assert_eq!(self.0.len(), 256);
        self.verify_pure(root, key, val)
    }

    /// Convenience function that returns whether or not the merkle branch is a correct proof of in/exclusion for a particular key-value binding.
    pub fn verify_unhashed(
        &self,
        root: tmelcrypt::HashVal,
        key: &impl Serialize,
        val: Option<&impl Serialize>,
    ) -> bool {
        let key = tmelcrypt::hash_single(&stdcode::serialize(&key).unwrap());
        if let Some(val) = val {
            let val = stdcode::serialize(val).unwrap();
            if self.verify(root, key, &val) {
                return true;
            }
        } else if self.verify(root, key, b"") {
            return true;
        }
        false
    }

    fn verify_pure(&self, root: tmelcrypt::HashVal, key: tmelcrypt::HashVal, val: &[u8]) -> bool {
        let path = key_to_path(key);
        let mut my_root = hash::datablock(val);
        for (&level, &direction) in self.0.iter().zip(path.iter()).rev() {
            if direction {
                my_root = hash::node(level, my_root)
            } else {
                my_root = hash::node(my_root, level)
            }
        }
        root == my_root
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
/// A compressed proof.
pub struct CompressedProof(pub Vec<u8>);

impl CompressedProof {
    /// Decompresses a compressed proof. Returns None if the format is invalid.
    pub fn decompress(&self) -> Option<FullProof> {
        let b = &self.0;
        if b.len() < 32 || b.len() % 32 != 0 {
            return None;
        }
        let bitmap = BitVec::<Msb0, u8>::from_slice(&b[..32]);
        let mut b = &b[32..];
        let mut out = Vec::new();
        // go through the bitmap. if b is set, insert a zero. otherwise, take 32 bytes from b. if b runs out, we are dead.
        for is_zero in bitmap {
            if is_zero {
                out.push(tmelcrypt::HashVal::default())
            } else {
                let mut buf = [0; 32];
                b.read_exact(&mut buf).ok()?;
                out.push(tmelcrypt::HashVal(buf));
            }
        }
        Some(FullProof(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;
    use tmelcrypt::HashVal;

    #[test]
    fn test_key_to_path_first() {
        let first = tmelcrypt::HashVal::default();
        let actual_first_path = key_to_path(first).to_vec();
        let expected_first_path = vec![false; 256];
        assert_eq!(actual_first_path, expected_first_path);
    }

    #[test]
    fn test_key_to_path_last() {
        let mut last = tmelcrypt::HashVal::default();
        last.0 = <[u8; 32]>::try_from(vec![u8::max_value(); 32]).unwrap();
        let actual_last_path = key_to_path(last).to_vec();
        let expected_last_path = vec![true; 256];
        assert_eq!(actual_last_path, expected_last_path);
    }

    #[test]
    fn test_decompress_none_when_length_is_zero() {
        // create empty vector

        // run decompress

        // expect none

        // Note that a length of zero requires its own check anc impl should probably be modified
        // ie instead of checking for l.t. expected size, jsut check length is non-zero and a multiple of
        // expected size
    }

    #[test]
    fn test_decompress_proof_none_when_length_not_multiple_of_expected_byte_size() {
        // create vectors full of random data ranging from
        // 1 to some upper bound greater than N * expected byte_size

        // iterate through each and if we have length % expected size == 0 continue
        // for all other values ensure we get None
    }

    #[test]
    fn test_decompress_proof_exists_when_length_is_multiple_of_expected_byte_size() {
        // create vectors full of random data ranging from
        // expected_byte_size * 1 to expected_byte_size*N

        // iterate through each and only ensure a proof exists
    }

    #[test]
    fn test_decompress_proof_is_valid() {
        // create vectors full of random data ranging from
        // expected_byte_size * 1 to expected_byte_size*N

        // iterate through each and only ensure a proof is valid and unique

        // Note: its hard to understand how this is decompression...
        // need better doc str on method...
    }

    #[test]
    fn test_all_header_bits_set() {
        // ...
    }

    #[test]
    fn test_decompress_proof_panic_on_buffer_read_fail() {
        // b.read_exact(&mut buf).ok()?;
        // in case there is an external failure while processing the buffer
        // (perhaps mock this somehow?) the method will panic / abort
    }

    #[test]
    fn test_compress_decompress_expected() {
        // create a compressed proof, from decompressing some arbitrary data...
        // ensure it matches the data the proof was generated from
        // keep doing that sequentially and ensure it matches
        // do that for various inputs and sizes (fuzz)
    }

    #[test]
    fn test_data_hashes() {
        // Unorganized notes to break into test cases
        // This may need higher-level fuzzing with the goal being with large enough values
        // we hit both data on rihgt and not conditional branches multiple times
        // Given random input they should be near 50/50 for high iterations
        //
        // TODO: consider moving compute out into a seperate function for test out notes outlined above
        // TODO: we need memoization / caching tests on this method to ensure we reset cache when we go over limit
        // This behavior seems somewhat strange.. shouldn't it be popping otu and removing oldest elements or
        // least important from cache instead of reseting teh entire thing?

        // Maybe its because teh cache code is commented out
    }

    #[test]
    fn test_verify_pure() {
        // TODO:
    }

    #[test]
    fn test_verify_unhashed() {
        // Is there a plan to use this in the future? It doesn't seem to be used anywhere in our code base
    }
}
