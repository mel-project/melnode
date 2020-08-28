use crate::common::*;
use lmdb::Transaction;
use lru::LruCache;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
//use std::path::Path;

/// Storage represents the persistent storage of the system.
pub struct Storage {
    pub curr_state: blkstructs::State,
    pub history: HashMap<u64, blkstructs::FinalizedState>,
    tree_db: autosmt::DBManager,

    recent_tx: LruCache<tmelcrypt::HashVal, ()>,

    lmdb_env: Arc<lmdb::Environment>,
    lmdb_db: lmdb::Database,
}

const GLOBAL_STATE_KEY: &[u8] = b"global_state";

impl Storage {
    /// Creates a new Storage for testing, with a genesis state that puts 1000 mel at the zero-zero coin, unlockable by the always_true script.
    pub fn open_testnet(path: &str) -> Result<Self> {
        let (lme, lmd) = open_lmdb(path)?;
        // load the db manager
        let dbm = autosmt::DBManager::load(autosmt::ondisk::LMDB::new(lme.clone(), None).unwrap());
        // recover the state
        let state = {
            let txn = lme.begin_ro_txn()?;
            match txn.get(lmd, &GLOBAL_STATE_KEY) {
                Ok(res) => {
                    log::debug!("loaded saved_global_state from LMDB");
                    let res = res.to_vec();
                    drop(txn);
                    blkstructs::State::from_partial_encoding_infallible(&res, &dbm)
                }
                Err(_) => {
                    drop(txn);
                    log::info!("creating a testnet genesis state from scratch");
                    new_genesis(dbm.clone())
                }
            }
        };
        let history = {
            let txn = lme.begin_ro_txn()?;
            let mut toret = HashMap::new();
            for height in (0..state.height).rev() {
                let key = format!("history_{}", height);
                if let Ok(res) = txn.get(lmd, &key.as_bytes()) {
                    let state =
                        blkstructs::FinalizedState::from_partial_encoding_infallible(&res, &dbm);
                    toret.insert(height, state);
                } else {
                    break;
                }
            }
            toret
        };
        Ok(Storage {
            curr_state: state,
            tree_db: dbm,
            history,

            recent_tx: LruCache::new(100000),

            lmdb_env: lme,
            lmdb_db: lmd,
        })
    }

    /// Inserts a new transaction.
    pub fn insert_tx(&mut self, tx: blkstructs::Transaction) -> Result<()> {
        let txhash = tx.hash_nosigs();
        if self.recent_tx.put(txhash, ()).is_some() {
            anyhow::bail!("already seen tx")
        }
        log::debug!(
            "attempting to apply tx {:?} onto state {:?}",
            txhash,
            self.curr_state.coins.root_hash()
        );
        self.curr_state.apply_tx(&tx)?;
        Ok(())
    }

    /// Syncs to disk.
    pub fn sync(&mut self) {
        self.tree_db.sync();
        log::debug!("saving global state");
        let mut txn = self.lmdb_env.begin_rw_txn().unwrap();
        txn.put(
            self.lmdb_db,
            &GLOBAL_STATE_KEY,
            &self.curr_state.partial_encoding(),
            lmdb::WriteFlags::empty(),
        )
        .unwrap();
        for (k, v) in self.history.iter() {
            let key = format!("history_{}", k);
            txn.put(
                self.lmdb_db,
                &key,
                &v.partial_encoding(),
                lmdb::WriteFlags::empty(),
            )
            .unwrap();
        }
        txn.commit().unwrap();
    }

    /// Gets a tx by the txhash.
    pub fn get_tx(&self, txhash: tmelcrypt::HashVal) -> Option<blkstructs::Transaction> {
        // first we try the current state
        if let (Some(tx), _) = self.curr_state.transactions.get(&txhash) {
            return Some(tx);
        }
        // nope that didn't work. scan through history
        // TODO do something intelligent
        for (_, s) in self.history.iter() {
            if let (Some(tx), _) = s.inner_ref().transactions.get(&txhash) {
                return Some(tx);
            }
        }
        None
    }

    /// Gets the last block.
    pub fn last_block(&self) -> Option<blkstructs::FinalizedState> {
        self.history.get(&(self.curr_state.height - 1)).cloned()
    }

    /// Consumes a block.
    pub fn apply_block(&mut self, blk: blkstructs::Block) -> Result<()> {
        if blk.header.height != self.curr_state.height {
            anyhow::bail!("apply_block wrong height");
        }
        let curr_height = self.curr_state.height;
        log::debug!(
            "apply_block at height {} with {} transactions",
            curr_height,
            blk.transactions.len()
        );
        let mut last_state = if self.curr_state.height == 0 {
            log::debug!("apply_block special case when height is zero");
            new_genesis(self.tree_db.clone())
        } else {
            self.history[&(curr_height - 1)].clone().next_state()
        };
        last_state.apply_tx_batch(&blk.transactions)?;
        let state = last_state.finalize();
        if state.header() != blk.header {
            anyhow::bail!("header mismatch");
        }
        self.history.insert(curr_height, state.clone());
        self.curr_state = state.next_state();
        self.sync();
        Ok(())
    }
}

fn open_lmdb(path: &str) -> Result<(Arc<lmdb::Environment>, lmdb::Database)> {
    let lmdb_env = lmdb::Environment::new()
        .set_max_dbs(1)
        .set_map_size(1 << 40)
        .open(Path::new(path))?;
    let db = lmdb_env.open_db(None)?;
    Ok((Arc::new(lmdb_env), db))
}

fn new_genesis(dbm: autosmt::DBManager) -> blkstructs::State {
    blkstructs::State::test_genesis(
        dbm,
        1000 * blkstructs::MICRO_CONVERTER,
        blkstructs::melscript::Script::always_true().hash(),
        (0..10)
            .map(|i| insecure_testnet_keygen(i).0)
            .collect::<Vec<_>>()
            .as_slice(),
    )
}
