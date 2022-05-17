use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
    time::Instant,
};

use std::time::Duration;

use clone_macro::clone;
use lru::LruCache;
use parking_lot::{Mutex, RwLock};
use smol::Task;
use stdcode::StdcodeSerializeExt;
use themelio_stf::{GenesisConfig, SealedState};
use themelio_structs::{Block, BlockHeight, CoinValue, ConsensusProof};

use super::{mempool::Mempool, MeshaCas};

/// Storage encapsulates all storage used by a Themelio full node (auditor or staker).
#[derive(Clone)]
pub struct Storage {
    mempool: Arc<RwLock<Mempool>>,
    metadata: boringdb::Dict,
    highest: Arc<RwLock<SealedState<MeshaCas>>>,
    old_cache: Arc<Mutex<LruCache<BlockHeight, SealedState<MeshaCas>>>>,
    forest: Arc<novasmt::Database<MeshaCas>>,

    _disk_sync: Arc<Task<()>>,
}

impl Storage {
    /// Gets an immutable reference to the mempool.
    pub fn mempool(&self) -> impl Deref<Target = Mempool> + '_ {
        self.mempool.read()
    }

    /// Gets a mutable reference to the mempool.
    pub fn mempool_mut(&self) -> impl DerefMut<Target = Mempool> + '_ {
        self.mempool.write()
    }

    /// Opens a NodeStorage, given a meshanina and boringdb database.
    pub fn new(mdb: meshanina::Mapping, bdb: boringdb::Database, genesis: GenesisConfig) -> Self {
        // Identify the genesis by the genesis ID
        let genesis_id = tmelcrypt::hash_single(stdcode::serialize(&genesis).unwrap());
        let metadata = bdb
            .open_dict(&format!("meta_genesis{}", genesis_id))
            .unwrap();
        let forest = novasmt::Database::new(MeshaCas::new(mdb));
        let highest = metadata
            .get(b"last_confirmed_block")
            .expect("db failed")
            .map(|b| {
                log::warn!("ACTUALLY LOADING DB");
                SealedState::from_block(&stdcode::deserialize(&b).unwrap(), &forest)
            })
            .unwrap_or_else(|| genesis.realize(&forest).seal(None));
        log::info!("HIGHEST AT {}", highest.inner_ref().height);

        let mempool = Arc::new(Mempool::new(highest.next_state()).into());
        let highest = Arc::new(RwLock::new(highest));
        let _disk_sync = smolscale::spawn(clone!([highest, forest, metadata], async move {
            loop {
                smol::Timer::after(Duration::from_secs(5)).await;
                let start = Instant::now();
                let highest = highest.read().clone();
                let forest = forest.clone();
                smol::unblock(move || forest.storage().flush()).await;
                metadata
                    .insert(
                        b"last_confirmed_block".to_vec(),
                        highest.to_block().stdcode(),
                    )
                    .unwrap();
                let elapsed = start.elapsed();
                if elapsed.as_secs() > 5 {
                    log::warn!("**** FLUSHED IN {:?} ****", elapsed);
                }
            }
        }))
        .into();
        Self {
            mempool,
            highest,
            forest: forest.into(),
            old_cache: Arc::new(LruCache::new(100).into()),
            metadata,
            _disk_sync,
        }
    }

    /// Obtain the highest state.
    pub fn highest_state(&self) -> SealedState<MeshaCas> {
        self.highest.read().deref().clone()
    }

    /// Obtain the highest height.
    pub fn highest_height(&self) -> BlockHeight {
        self.highest.read().inner_ref().height
    }

    /// Obtain a historical SealedState.
    pub fn get_state(&self, height: BlockHeight) -> Option<SealedState<MeshaCas>> {
        let highest = self.highest_state();
        if height == highest.inner_ref().height {
            return Some(highest);
        }
        let old = self.old_cache.lock().get(&height).cloned();
        if let Some(old) = old {
            Some(old)
        } else {
            let old_blob = self
                .metadata
                .get(format!("block-{}", height).as_bytes())
                .unwrap()?;
            let old_state =
                SealedState::from_block(&stdcode::deserialize(&old_blob).unwrap(), &self.forest);
            self.old_cache.lock().put(height, old_state.clone());
            Some(old_state)
        }
    }

    /// Obtain a historical ConsensusProof.
    pub fn get_consensus(&self, height: BlockHeight) -> Option<ConsensusProof> {
        let height = self
            .metadata
            .get(format!("cproof-{}", height).as_bytes())
            .unwrap()?;
        stdcode::deserialize(&height).ok()
    }

    /// Consumes a block, applying it to the current state.
    pub async fn apply_block(&self, blk: Block, cproof: ConsensusProof) -> anyhow::Result<()> {
        let highest_state = self.highest_state();
        let header = blk.header;
        if header.height != highest_state.inner_ref().height + 1.into() {
            anyhow::bail!(
                "cannot apply block {} to height {}",
                header.height,
                highest_state.inner_ref().height
            );
        }

        // Check the consensus proof
        let mut total_votes = CoinValue(0);
        let mut present_votes = CoinValue(0);
        for stake_doc in highest_state.inner_ref().stakes.val_iter() {
            if blk.header.height.epoch() >= stake_doc.e_start
                && blk.header.height.epoch() < stake_doc.e_post_end
            {
                total_votes += stake_doc.syms_staked;
                if let Some(v) = cproof.get(&stake_doc.pubkey) {
                    if stake_doc.pubkey.verify(&blk.header.hash(), v) {
                        present_votes += total_votes;
                    }
                }
            }
        }
        if present_votes.0 <= 2 * total_votes.0 / 3 {
            anyhow::bail!(
                "rejecting putative block {} due to insufficient votes ({}/{})",
                blk.header.height,
                present_votes,
                total_votes
            )
        }

        let start = Instant::now();
        let new_state = highest_state.apply_block(&blk)?;
        let blkbytes = new_state.to_block().stdcode();
        let apply_time = start.elapsed();
        let start = Instant::now();
        let blklen = blkbytes.len();
        self.metadata.insert(
            format!("block-{}", new_state.inner_ref().height)
                .as_bytes()
                .to_vec(),
            blkbytes,
        )?;
        self.metadata.insert(
            format!("cproof-{}", new_state.inner_ref().height)
                .as_bytes()
                .to_vec(),
            stdcode::serialize(&cproof)?,
        )?;
        log::debug!(
            "applied block {} of length {} in {:.2}ms (insert {:.2}ms)",
            new_state.inner_ref().height,
            blklen,
            apply_time.as_secs_f64() * 1000.0,
            start.elapsed().as_secs_f64() * 1000.0
        );
        *self.highest.write() = new_state;
        let next = self.highest_state().next_state();
        self.mempool_mut().rebase(next);
        Ok(())
    }

    /// Gets the forest.
    pub fn forest(&self) -> &novasmt::Database<MeshaCas> {
        &self.forest
    }
}
