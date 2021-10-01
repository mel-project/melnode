#![allow(clippy::upper_case_acronyms)]

mod mempool;
mod smt;
use std::sync::Arc;

use self::mempool::Mempool;
use blkdb::{traits::DbBackend, BlockTree};
use parking_lot::RwLock;
pub use smt::*;
use themelio_nodeprot::TrustStore;
use themelio_stf::{BlockHeight, ConsensusProof, GenesisConfig, SealedState};

#[derive(Clone)]
pub struct NodeTrustStore(pub SharedStorage);

impl TrustStore for NodeTrustStore {
    fn set(&self, netid: themelio_stf::NetID, trusted: themelio_nodeprot::TrustedHeight) {
        self.0
            .read()
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
            .read()
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
pub type SharedStorage = Arc<RwLock<NodeStorage>>;

/// NodeStorage encapsulates all storage used by a Themelio full node (auditor or staker).
pub struct NodeStorage {
    mempool: Mempool,
    metadata: boringdb::Dict,

    history: BlockTree<BoringDbBackend>,
    forest: novasmt::Forest,
}

impl NodeStorage {
    /// Gets an immutable reference to the mempool.
    pub fn mempool(&self) -> &Mempool {
        &self.mempool
    }

    /// Gets a mutable reference to the mempool.
    pub fn mempool_mut(&mut self) -> &mut Mempool {
        &mut self.mempool
    }

    /// Opens a NodeStorage, given a sled database.
    pub fn new(db: boringdb::Database, genesis: GenesisConfig) -> Self {
        // Identify the genesis by the genesis ID
        let genesis_id = tmelcrypt::hash_single(stdcode::serialize(&genesis).unwrap());
        let dict = db.open_dict(&format!("genesis{}", genesis_id)).unwrap();
        let metadata = db
            .open_dict(&format!("meta_genesis{}", genesis_id))
            .unwrap();
        let forest = novasmt::Forest::new(BoringDbSmt::new(dict.clone()));
        let mut history = BlockTree::new(BoringDbBackend { dict }, forest.clone(), true);

        // initialize stuff
        if history.get_tips().is_empty() {
            history.set_genesis(genesis.realize(&forest).seal(None), &[]);
        }

        let mempool_state = history.get_tips()[0].to_state().next_state();
        Self {
            mempool: Mempool::new(mempool_state),
            history,
            forest,
            metadata,
        }
    }

    /// Obtain the highest state.
    pub fn highest_state(&self) -> SealedState {
        self.get_state(self.highest_height()).unwrap()
    }

    /// Obtain the highest height.
    pub fn highest_height(&self) -> BlockHeight {
        let tips = self.history.get_tips();
        if tips.len() != 1 {
            #[cfg(not(feature = "metrics"))]
            log::error!(
                "multiple tips: {:#?}",
                tips.iter().map(|v| v.header()).collect::<Vec<_>>()
            );
            #[cfg(feature = "metrics")]
            log::error!(
                "hostname={} public_ip={} multiple tips: {:#?}",
                crate::prometheus::HOSTNAME.as_str(), crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
                tips.iter().map(|v| v.header()).collect::<Vec<_>>()
            );

        }
        tips.into_iter().map(|v| v.header().height).max().unwrap()
    }

    /// Obtain a historical SealedState.
    pub fn get_state(&self, height: BlockHeight) -> Option<SealedState> {
        self.history
            .get_at_height(height)
            .get(0)
            .map(|v| v.to_state())
    }

    /// Obtain a historical ConsensusProof.
    pub fn get_consensus(&self, height: BlockHeight) -> Option<ConsensusProof> {
        let height = self.history.get_at_height(height).into_iter().next()?;
        stdcode::deserialize(height.metadata()).ok()
    }

    /// Consumes a block, applying it to the current state.
    pub fn apply_block(
        &mut self,
        blk: themelio_stf::Block,
        cproof: ConsensusProof,
    ) -> anyhow::Result<()> {
        let highest_height = self.highest_height();
        if blk.header.height != highest_height + 1.into() {
            anyhow::bail!(
                "cannot apply block {} to height {}",
                blk.header.height,
                highest_height
            );
        }

        self.history
            .apply_block(&blk, &stdcode::serialize(&cproof).unwrap())?;
        #[cfg(not(feature = "metrics"))]
        log::debug!("applied block {}", blk.header.height);
        #[cfg(feature = "metrics")]
        log::debug!("hostname={} public_ip={} applied block {}", crate::prometheus::HOSTNAME.as_str(), crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(), blk.header.height);
        let next = self.highest_state().next_state();
        self.mempool_mut().rebase(next);
        Ok(())
    }

    /// Convenience method to "share" storage.
    pub fn share(self) -> SharedStorage {
        Arc::new(RwLock::new(self))
    }

    /// Gets the forest.
    pub fn forest(&self) -> novasmt::Forest {
        self.forest.clone()
    }

    /// Gets the blockdb.
    pub fn history_mut(&mut self) -> &mut blkdb::BlockTree<impl DbBackend> {
        &mut self.history
    }
}

struct BoringDbBackend {
    dict: boringdb::Dict,
}

impl DbBackend for BoringDbBackend {
    fn insert(&mut self, key: &[u8], value: &[u8]) -> Option<Vec<u8>> {
        self.dict
            .insert(key.to_vec(), value.to_vec())
            .unwrap()
            .map(|v| v.to_vec())
    }

    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.dict.get(key).unwrap().map(|v| v.to_vec())
    }

    fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        self.dict.remove(key).unwrap().map(|v| v.to_vec())
    }

    fn key_range(&self, start: &[u8], end: &[u8]) -> Vec<Vec<u8>> {
        self.dict
            .range(start..=end)
            .unwrap()
            .map(|v| v.unwrap().0.to_vec())
            .collect()
    }
}