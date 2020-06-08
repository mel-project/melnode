use blake2b_simd::Params;
use std::cell::RefCell;

thread_local!(pub static HASH_COUNT: RefCell<u64> = RefCell::new(0));

pub fn datablock(bytes: &[u8]) -> [u8; 32] {
    if bytes.is_empty() {
        return empty_hash();
    }
    HASH_COUNT.with(|hc| {
        let mut nval = hc.borrow_mut();
        *nval += 1
    });
    *blake3::hash(bytes).as_bytes()
}

pub fn index(bytes: &[u8]) -> [u8; 32] {
    let hash = Params::new().hash_length(32).key(b"index").hash(bytes);
    let bytes = hash.as_bytes();
    let mut ret: [u8; 32] = [0; 32];
    ret.copy_from_slice(&bytes);
    ret
}

pub fn node(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    // println!(
    //     "node({}, {})",
    //     hex::encode(&left[..5]),
    //     hex::encode(&right[..5])
    // );
    let empty = [0; 32];
    if left == empty && right == empty {
        return empty_hash();
    }
    let mut v: Vec<u8> = Vec::with_capacity(64);
    v.append(&mut left.to_vec());
    v.append(&mut right.to_vec());
    // println!(
    //     "node(left={}, right={})",
    //     hex::encode(&left[..4]),
    //     hex::encode(&right[..4])
    // );
    datablock(&v)
}

pub fn node_appended(everything: Vec<u8>) -> [u8; 32] {
    if everything == vec![0; everything.len()] {
        return [0; 32];
    }
    datablock(&everything)
}

pub fn empty_hash() -> [u8; 32] {
    // datablock(&vec![])
    [0; 32]
}
