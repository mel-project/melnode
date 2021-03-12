pub fn get_data_block_hash_val(bytes: &[u8]) -> tmelcrypt::HashVal {
    if bytes.is_empty() {
        tmelcrypt::HashVal::default()
    } else {
        tmelcrypt::hash_keyed(b"smt_datablock", bytes)
    }
}

pub fn get_node_hash_val(left: tmelcrypt::HashVal, right: tmelcrypt::HashVal) -> tmelcrypt::HashVal {
    if left == tmelcrypt::HashVal::default() && right == tmelcrypt::HashVal::default() {
        return tmelcrypt::HashVal::default();
    }
    let mut v: Vec<u8> = Vec::with_capacity(64);
    v.extend_from_slice(&left.0);
    v.extend_from_slice(&right.0);
    tmelcrypt::hash_keyed(b"smt_node", &v)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datablock_empty() {

    }

    #[test]
    fn test_datablock_not_empty() {

    }
}