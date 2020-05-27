pub mod ondisk;
pub mod smt;

pub use smt::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_tree() {
        let db = wrap_db(TrivialDB::new());
        let empty_tree = Tree::new(&db);
        assert_eq!(empty_tree.root_hash(), [0; 32]);
    }

    #[test]
    fn simple_tree() {
        let db = wrap_db(TrivialDB::new());
        {
            let mut tree = Tree::new(&db);
            for i in 0..10 {
                tree = tree.set(
                    hash::index(format!("key-{}", i).as_bytes()),
                    &hash::index(format!("val-{}", i).as_bytes()),
                );
            }
            // successfully built tree
            assert_eq!(
                tree.root_hash().to_vec(),
                hex::decode("90822fba8e6113467241091a14fc3eb359d0815dd64d1b091c573215f2a621dc")
                    .unwrap()
            );
        }
        // everything is freed at the end
        assert_eq!(db.read().unwrap().count(), 0);
    }
}
