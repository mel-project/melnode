use blkstructs::{ConfirmedState, ConsensusProof, SealedState, Transaction};
use lru::LruCache;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tmelcrypt::HashVal;
use tracing::instrument;

use super::insecure_testnet_keygen;

/// Locked storage.
pub type SharedStorage = Arc<RwLock<Storage>>;

/// Storage represents the persistent storage of the system.
pub struct Storage {
    /// Provisional state that is used as a "mempool"
    pub provis_state: Option<blkstructs::State>,

    postconfirm_state: blkstructs::State,
    history: HashMap<u64, ConfirmedState>,
    tree_db: autosmt::DBManager,

    recent_tx: LruCache<HashVal, Transaction>,

    lmdb_env: Arc<lmdb::Environment>,
    lmdb_db: lmdb::Database,
}

const GLOBAL_STATE_KEY: &[u8] = b"global_state";

impl neosymph::TxLookup for Storage {
    fn lookup(&self, hash: HashVal) -> Option<Transaction> {
        self.get_tx(hash)
    }
}

impl Storage {
    /// Creates a new Storage for testing, with a genesis state that puts 1000 mel at the zero-zero coin, unlockable by the always_true script.
    #[instrument]
    pub fn open_testnet(path: &str) -> anyhow::Result<Self> {
        use lmdb::Transaction;
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
            let mut last_state: Option<SealedState>;
            for height in (0..state.height).rev() {
                let key = format!("history_{}", height);
                if let Ok(res) = txn.get(lmd, &key.as_bytes()) {
                    log::debug!("loading history at height {}...", height);
                    let state =
                        blkstructs::SealedState::from_partial_encoding_infallible(&res, &dbm);
                    last_state = Some(state.clone());
                    let proof_key = format!("proof_{}", height);
                    let proof = stdcode::deserialize(&txn.get(lmd, &proof_key.as_bytes()).unwrap())
                        .unwrap();
                    let lala = last_state.map(|fs| fs.inner_ref().clone());
                    toret.insert(height, state.confirm(proof, lala.as_ref()).unwrap());
                } else {
                    break;
                }
            }
            toret
        };
        Ok(Storage {
            provis_state: None,

            postconfirm_state: state,
            tree_db: dbm,
            history,

            recent_tx: LruCache::new(100000),

            lmdb_env: lme,
            lmdb_db: lmd,
        })
    }

    /// Gets a historical item
    pub fn get_history(&self, height: u64) -> Option<&ConfirmedState> {
        self.history.get(&height)
    }

    /// Inserts a new transaction.
    #[instrument(skip(self))]
    pub fn insert_tx(&mut self, tx: blkstructs::Transaction) -> anyhow::Result<()> {
        let state = self
            .provis_state
            .as_mut()
            .unwrap_or(&mut self.postconfirm_state);
        let txhash = tx.hash_nosigs();
        if self.recent_tx.put(txhash, tx.clone()).is_some() {
            anyhow::bail!("already seen tx")
        }
        log::debug!(
            "attempting to apply tx {:?} onto state {:?}",
            txhash,
            state.coins.root_hash()
        );
        state.apply_tx(&tx)?;
        Ok(())
    }

    /// Syncs to disk.
    #[instrument(skip(self))]
    pub fn sync(&mut self) {
        use lmdb::Transaction;
        self.tree_db.sync();
        log::debug!("saving global state");
        let mut txn = self.lmdb_env.begin_rw_txn().unwrap();
        txn.put(
            self.lmdb_db,
            &GLOBAL_STATE_KEY,
            &self.postconfirm_state.partial_encoding(),
            lmdb::WriteFlags::empty(),
        )
        .unwrap();
        for (k, v) in self.history.iter() {
            let key = format!("history_{}", k);
            txn.put(
                self.lmdb_db,
                &key,
                &v.inner().partial_encoding(),
                lmdb::WriteFlags::empty(),
            )
            .unwrap();
            let proof_key = format!("proof_{}", k);
            txn.put(
                self.lmdb_db,
                &proof_key,
                &stdcode::serialize(v.cproof()).unwrap(),
                lmdb::WriteFlags::empty(),
            )
            .unwrap();
        }
        txn.commit().unwrap();
    }

    /// Gets a tx by the txhash.
    #[instrument(skip(self))]
    pub fn get_tx(&self, txhash: tmelcrypt::HashVal) -> Option<blkstructs::Transaction> {
        // first we try the cache
        if let Some(val) = self.recent_tx.peek(&txhash) {
            return Some(val.clone());
        }
        // first we try the current state
        if let (Some(tx), _) = self.postconfirm_state.transactions.get(&txhash) {
            return Some(tx);
        }
        // nope that didn't work. scan through history
        // TODO do something intelligent
        for (_, s) in self.history.iter() {
            if let (Some(tx), _) = s.inner().inner_ref().transactions.get(&txhash) {
                return Some(tx);
            }
        }
        None
    }

    /// Gets the last block.
    #[instrument(skip(self))]
    pub fn last_block(&self) -> Option<blkstructs::ConfirmedState> {
        self.history
            .get(&(self.postconfirm_state.height.checked_sub(1)?))
            .cloned()
    }

    /// Last state.
    pub fn genesis(&self) -> SealedState {
        new_genesis(self.tree_db.clone()).seal(None)
    }

    /// Consumes a block.
    #[instrument(skip(self, blk, cproof))]
    pub fn apply_confirmed_block(
        &mut self,
        blk: blkstructs::Block,
        cproof: ConsensusProof,
    ) -> anyhow::Result<()> {
        if blk.header.height != self.postconfirm_state.height {
            anyhow::bail!(
                "apply_block wrong height {} {}",
                blk.header.height,
                self.postconfirm_state.height
            );
        }
        let curr_height = self.postconfirm_state.height;
        log::debug!(
            "apply_block at height {} with {} transactions",
            curr_height,
            blk.transactions.len()
        );
        let mut last_state = if self.postconfirm_state.height == 0 {
            log::debug!("apply_block special case when height is zero");
            new_genesis(self.tree_db.clone())
        } else {
            self.history[&(curr_height - 1)]
                .clone()
                .inner()
                .next_state()
        };
        last_state.apply_tx_batch(&blk.transactions.iter().cloned().collect::<Vec<_>>())?;
        let state = last_state.seal(blk.proposer_action);
        if state.header() != blk.header {
            anyhow::bail!(
                "header mismatch! got {:#?}, expected {:#?}, after applying block {:#?}",
                state.header(),
                blk.header,
                blk
            );
        }
        self.history.insert(
            curr_height,
            state
                .clone()
                .confirm(cproof, Some(state.inner_ref()))
                .ok_or_else(|| anyhow::anyhow!("incorrect proof"))?,
        );
        self.postconfirm_state = state.next_state();
        log::debug!(
            "block {}, txcount={}, hash={:?} APPLIED",
            curr_height,
            blk.transactions.len(),
            blk.header.hash()
        );
        // self.sync();
        Ok(())
    }
}

#[instrument]
fn open_lmdb(path: &str) -> anyhow::Result<(Arc<lmdb::Environment>, lmdb::Database)> {
    let lmdb_env = lmdb::Environment::new()
        .set_max_dbs(1)
        .set_map_size(1 << 40)
        .open(Path::new(path))?;
    let db = lmdb_env.open_db(None)?;
    Ok((Arc::new(lmdb_env), db))
}

#[instrument(skip(dbm))]
fn new_genesis(dbm: autosmt::DBManager) -> blkstructs::State {
    blkstructs::State::test_genesis(
        dbm,
        1000 * blkstructs::MICRO_CONVERTER,
        blkstructs::melvm::Covenant::always_true().hash(),
        (0..2)
            .map(|i| insecure_testnet_keygen(i).0)
            .collect::<Vec<_>>()
            .as_slice(),
    )
}
