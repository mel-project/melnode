use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
    time::Instant,
};
use event_listener::Event;

use moka::sync::Cache;
use parking_lot::RwLock;

use themelio_stf::{GenesisConfig, SealedState};
use themelio_structs::{Block, BlockHeight, CoinValue, ConsensusProof};

use super::{history::History, mempool::Mempool, MeshaCas};

/// Storage encapsulates all storage used by a Themelio full node (auditor or staker).
#[derive(Clone)]
pub struct Storage {
    history: Arc<History>,

    mempool: Arc<RwLock<Mempool>>,
    highest: Arc<RwLock<SealedState<MeshaCas>>>,
    old_cache: Arc<Cache<BlockHeight, SealedState<MeshaCas>>>,
    forest: Arc<novasmt::Database<MeshaCas>>,

    /// A notifier for a new block happening.
    new_block_notify: Arc<Event>,
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
    pub fn new(mdb: meshanina::Mapping, history: History, genesis: GenesisConfig) -> Self {
        // Identify the genesis by the genesis ID
        let history = Arc::new(history);

        let forest = novasmt::Database::new(MeshaCas::new(mdb));

        let highest = history
            .get_block(history.highest())
            .expect("cannot get highest")
            .map(|s| SealedState::from_block(&s.0, &forest))
            .unwrap_or_else(|| genesis.realize(&forest).seal(None));
        log::info!("HIGHEST AT {}", highest.inner_ref().height);

        let mempool = Arc::new(Mempool::new(highest.next_state()).into());
        let highest = Arc::new(RwLock::new(highest));
        Self {
            mempool,
            highest,
            forest: forest.into(),
            old_cache: Arc::new(Cache::new(1000)),
            history,
            new_block_notify: Default::default(),
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

    /// Waits until a certain height is available, then returns it.
    pub async fn get_state_or_wait(&self, height: BlockHeight) -> SealedState<MeshaCas> {
        loop {
            let evt = self.new_block_notify.listen();
            if let Some(val) = self.get_state(height) {
                return val;
            }
            evt.await;
        }
    }

    /// Obtain a historical SealedState.
    pub fn get_state(&self, height: BlockHeight) -> Option<SealedState<MeshaCas>> {
        let highest = self.highest_state();
        if height == highest.inner_ref().height {
            return Some(highest);
        }
        if height > highest.inner_ref().height {
            return None;
        }
        let old = self.old_cache.get(&height);
        if let Some(old) = old {
            Some(old)
        } else {
            let (old_block, _) = self
                .history
                .get_block(height)
                .expect("failed to get older block")?;
            let old_state = SealedState::from_block(&old_block, &self.forest);
            self.old_cache.insert(height, old_state.clone());
            Some(old_state)
        }
    }

    /// Obtain a historical ConsensusProof.
    pub fn get_consensus(&self, height: BlockHeight) -> Option<ConsensusProof> {
        Some(
            self.history
                .get_block(height)
                .expect("cannot get older block")?
                .1,
        )
    }

    /// Synchronizes everything to disk.
    pub async fn flush(&self) {
        let forest = self.forest.clone();
        let history = self.history.clone();
        smol::unblock(move || {
            history.flush().unwrap();
            forest.storage().flush();
        })
        .await;
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
        let apply_time = start.elapsed();
        let start = Instant::now();
        self.history
            .insert_block(&new_state.to_block(), &cproof)
            .unwrap();
        log::debug!(
            "applied block {} in {:.2}ms (insert {:.2}ms)",
            new_state.inner_ref().height,
            apply_time.as_secs_f64() * 1000.0,
            start.elapsed().as_secs_f64() * 1000.0
        );
        *self.highest.write() = new_state;
        let next = self.highest_state().next_state();
        self.mempool_mut().rebase(next);
        self.new_block_notify.notify(usize::MAX);

        if fastrand::usize(0..3000) == 0 {
            self.flush().await;
        }
        Ok(())
    }

    /// Gets the forest.
    pub fn forest(&self) -> &novasmt::Database<MeshaCas> {
        &self.forest
    }
}
