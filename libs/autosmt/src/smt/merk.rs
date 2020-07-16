use crate::smt::*;

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
