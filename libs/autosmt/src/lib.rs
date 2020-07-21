pub mod ondisk;
pub mod smt;

pub use smt::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_tree() {
        let db = MemDB::default();
        let db = DBManager::load(db);
        let tree = db.get_tree(tmelcrypt::HashVal::default());
        assert_eq!(tree.root_hash(), tmelcrypt::HashVal::default());
    }

    #[test]
    fn simple_tree() {
        let db = DBManager::load(MemDB::default());
        {
            let mut tree = db.get_tree(tmelcrypt::HashVal::default());
            for i in 0..10 {
                let key = tmelcrypt::hash_single(format!("key-{}", i).as_bytes());
                let val = tmelcrypt::hash_single(format!("val-{}", i).as_bytes()).to_vec();
                tree = tree.set(
                    tmelcrypt::hash_single(format!("key-{}", i).as_bytes()),
                    &val,
                );
                dbg!(tree.get(key));
                assert_eq!(tree.get(key).0, val);
            }
            // successfully built tree
            dbg!(tree.root_hash());
        }
    }

    #[test]
    fn iterator() {
        let db = DBManager::load(MemDB::default());
        let mut tree = db.get_tree(tmelcrypt::HashVal::default());
        let mut mapping = std::collections::HashMap::new();
        for i in 0..10 {
            let key = tmelcrypt::hash_single(format!("key-{}", i).as_bytes());
            let val = tmelcrypt::hash_single(format!("val-{}", i).as_bytes()).to_vec();
            tree = tree.set(
                tmelcrypt::hash_single(format!("key-{}", i).as_bytes()),
                &val,
            );
            mapping.insert(key, val);
        }
        for (k, v) in tree.iter() {
            assert_eq!(mapping[&k], v)
        }
    }
}
