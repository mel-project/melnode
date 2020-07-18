pub fn datablock(bytes: &[u8]) -> tmelcrypt::HashVal {
    if bytes.is_empty() {
        tmelcrypt::HashVal::default()
    } else {
        tmelcrypt::hash_keyed(b"smt_datablock", bytes)
    }
}

pub fn node(left: tmelcrypt::HashVal, right: tmelcrypt::HashVal) -> tmelcrypt::HashVal {
    if left == tmelcrypt::HashVal::default() && right == tmelcrypt::HashVal::default() {
        return tmelcrypt::HashVal::default();
    }
    let mut v: Vec<u8> = Vec::with_capacity(64);
    v.extend_from_slice(&left.0);
    v.extend_from_slice(&right.0);
    tmelcrypt::hash_keyed(b"smt_node", &v)
}
