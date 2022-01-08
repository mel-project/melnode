#![allow(clippy::upper_case_acronyms)]

mod mempool;
mod smt;
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
    time::Instant,
};

use self::mempool::Mempool;

use anyhow::Context;
use arc_swap::ArcSwap;
use dashmap::DashMap;
use parking_lot::RwLock;
pub use smt::*;

use themelio_nodeprot::TrustStore;
use themelio_stf::{BlockHeight, ConsensusProof, GenesisConfig, SealedState};

#[derive(Clone)]
pub struct NodeTrustStore(pub SharedStorage);

impl TrustStore for NodeTrustStore {
    fn set(&self, netid: themelio_stf::NetID, trusted: themelio_nodeprot::TrustedHeight) {
        self.0
            .metadata
            .insert(
                stdcode::serialize(&netid).expect("cannot serialize netid"),
                stdcode::serialize(&(trusted.height, trusted.header_hash))
                    .expect("Cannot serialize trusted height"),
            )
            .expect("could not set trusted height");
    }

    fn get(&self, netid: themelio_stf::NetID) -> Option<themelio_nodeprot::TrustedHeight> {
        let pair: (BlockHeight, tmelcrypt::HashVal) = self
            .0
            .metadata
            .get(&stdcode::serialize(&netid).expect("cannot serialize netid"))
            .expect("cannot get")
            .map(|b| stdcode::deserialize(&b).expect("cannot deserialize"))?;
        Some(themelio_nodeprot::TrustedHeight {
            height: pair.0,
            header_hash: pair.1,
        })
    }
}

/// An alias for a shared NodeStorage.
pub type SharedStorage = Arc<NodeStorage>;

/// NodeStorage encapsulates all storage used by a Themelio full node (auditor or staker).
pub struct NodeStorage {
    mempool: RwLock<Mempool>,
    metadata: boringdb::Dict,
    highest: ArcSwap<SealedState<MeshaCas>>,
    old_cache: DashMap<BlockHeight, SealedState<MeshaCas>>,
    forest: novasmt::Database<MeshaCas>,
}

impl NodeStorage {
    /// Gets an immutable reference to the mempool.
    pub fn mempool(&self) -> impl Deref<Target = Mempool> + '_ {
        self.mempool.read()
    }

    /// Gets a mutable reference to the mempool.
    pub fn mempool_mut(&self) -> impl DerefMut<Target = Mempool> + '_ {
        self.mempool.write()
    }

    /// Opens a NodeStorage, given a sled database.
    pub fn new(mdb: meshanina::Mapping, bdb: boringdb::Database, genesis: GenesisConfig) -> Self {
        // Identify the genesis by the genesis ID
        let genesis_id = tmelcrypt::hash_single(stdcode::serialize(&genesis).unwrap());
        let metadata = bdb
            .open_dict(&format!("meta_genesis{}", genesis_id))
            .unwrap();
        let forest = novasmt::Database::new(MeshaCas::new(mdb));
        let highest = metadata
            .get(b"last_confirmed")
            .expect("db failed")
            .map(|b| SealedState::from_partial_encoding_infallible(&b, &forest))
            .unwrap_or_else(|| genesis.realize(&forest).seal(None));
        Self {
            mempool: Mempool::new(highest.next_state()).into(),
            highest: Arc::new(highest).into(),
            forest,
            old_cache: Default::default(),
            metadata,
        }
    }

    /// Obtain the highest state.
    pub fn highest_state(&self) -> SealedState<MeshaCas> {
        self.highest.load_full().deref().clone()
    }

    /// Obtain the highest height.
    pub fn highest_height(&self) -> BlockHeight {
        self.highest.load().inner_ref().height
    }

    /// Obtain a historical SealedState.
    pub fn get_state(&self, height: BlockHeight) -> Option<SealedState<MeshaCas>> {
        self.old_cache
            .entry(height)
            .or_try_insert_with(|| {
                let old_blob = self
                    .metadata
                    .get(format!("state-{}", height).as_bytes())
                    .unwrap()
                    .context("no such height")?;
                let old_state =
                    SealedState::from_partial_encoding_infallible(&old_blob, &self.forest);
                Ok::<_, anyhow::Error>(old_state)
            })
            .ok()
            .map(|r| r.clone())
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
    pub fn apply_block(
        &self,
        blk: themelio_stf::Block,
        cproof: ConsensusProof,
    ) -> anyhow::Result<()> {
        let highest_state = self.highest_state();
        if blk.header.height != highest_state.inner_ref().height + 1.into() {
            anyhow::bail!(
                "cannot apply block {} to height {}",
                blk.header.height,
                highest_state.inner_ref().height
            );
        }
        // TODO!!!! CHECK INTEGRITY?!!?!?!!
        let new_state = highest_state.apply_block(&blk)?;
        self.metadata.insert(
            format!("state-{}", new_state.inner_ref().height)
                .as_bytes()
                .to_vec(),
            new_state.partial_encoding(),
        )?;
        self.metadata.insert(
            format!("cproof-{}", new_state.inner_ref().height)
                .as_bytes()
                .to_vec(),
            stdcode::serialize(&cproof)?,
        )?;
        self.highest.store(new_state.into());
        #[cfg(not(feature = "metrics"))]
        log::debug!("applied block {}", blk.header.height);
        #[cfg(feature = "metrics")]
        log::debug!(
            "hostname={} public_ip={} applied block {}",
            crate::prometheus::HOSTNAME.as_str(),
            crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
            blk.header.height
        );
        let next = self.highest_state().next_state();
        self.mempool_mut().rebase(next);
        Ok(())
    }

    /// Convenience method to "share" storage.
    pub fn share(self) -> SharedStorage {
        let toret = Arc::new(self);
        let copy = toret.clone();
        // start a background thread to periodically sync
        std::thread::Builder::new()
            .name("storage-sync".into())
            .spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(30));
                let start = Instant::now();
                let highest = copy.highest_state();
                let forest = copy.forest().clone();
                forest.storage().flush();
                copy.metadata
                    .insert(b"last_confirmed".to_vec(), highest.partial_encoding())
                    .unwrap();
                log::warn!("**** FLUSHED IN {:?} ****", start.elapsed());
            })
            .unwrap();
        toret
    }

    /// Gets the forest.
    pub fn forest(&self) -> novasmt::Database<MeshaCas> {
        self.forest.clone()
    }
}
