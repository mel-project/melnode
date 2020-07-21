use crate::common::*;
use anyhow::Result;
use smol::*;

/// Storage represents the persistent storage of the system.
pub struct Storage {
    curr_state: blkstructs::State,
    tree_db: autosmt::DBManager,
}

impl Storage {
    /// Creates a new Storage for testing, with an in-memory genesis state that puts 1000 mel at the zero-zero coin, unlockable by the always_true script.
    pub fn new_test() -> Self {
        let db = autosmt::DBManager::load(autosmt::MemDB::default());
        let state = blkstructs::State::test_genesis(
            db.clone(),
            blkstructs::MICRO_CONVERTER * 1000,
            blkstructs::melscript::Script::always_true().hash(),
        );
        Storage {
            curr_state: state,
            tree_db: db,
        }
    }

    /// Inserts a new transaction.
    pub fn insert_tx(&mut self, tx: blkstructs::Transaction) -> Result<()> {
        let txhash = tx.hash_nosigs();
        log::debug!(
            "attempting to apply tx {:?} onto state {:?}",
            txhash,
            self.curr_state.coins.root_hash()
        );
        self.curr_state.apply_tx(&tx)?;
        Ok(())
    }
}
