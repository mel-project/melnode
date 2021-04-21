pub(crate) const DATA_BLOCK_HASH_KEY: &[u8; 13] = b"smt_datablock";
pub(crate) const NODE_HASH_KEY: &[u8; 8] = b"smt_node";

pub fn datablock(bytes: &[u8]) -> tmelcrypt::HashVal {
    if bytes.is_empty() {
        tmelcrypt::HashVal::default()
    } else {
        tmelcrypt::hash_keyed(DATA_BLOCK_HASH_KEY, bytes)
    }
}

pub fn node(left: tmelcrypt::HashVal, right: tmelcrypt::HashVal) -> tmelcrypt::HashVal {
    if left == tmelcrypt::HashVal::default() && right == tmelcrypt::HashVal::default() {
        return tmelcrypt::HashVal::default();
    }
    let mut v: Vec<u8> = Vec::with_capacity(64);
    v.extend_from_slice(&left.0);
    v.extend_from_slice(&right.0);
    tmelcrypt::hash_keyed(NODE_HASH_KEY, &v)
}

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn test_datablock_empty() {
        // call with empty slice and check for expected default
    }

    #[test]
    fn test_datablock_not_empty() {
        // call with multiple non-empty slices and check for expected value
        // ensure no duplicates / unique
    }
}
