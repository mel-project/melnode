mod constants;
mod melscript;
mod state;
mod transaction;
pub use constants::*;
pub use transaction::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn smt_mapping() {
        let tree = autosmt::Tree::new(&autosmt::wrap_db(autosmt::TrivialDB::new()));
        let mut map: state::SmtMapping<u64, u64, autosmt::TrivialDB> =
            state::SmtMapping::new(&tree);
        for i in 0..10 {
            map.insert(&i, &i);
        }
        assert_eq!(
            hex::encode(map.mapping.root_hash()),
            "c817ba6ba9cadabb754ed5195232be8d22dbd98a1eeca0379921c3cc0b414110"
        );
        for i in 0..10 {
            assert_eq!(Some(i), map.get(&i).0);
        }
        map.delete(&5);
        assert_eq!(None, map.get(&5).0);
        for i in 0..10 {
            map.delete(&i);
        }
        assert_eq!(map.mapping.root_hash(), [0; 32]);
    }
}
