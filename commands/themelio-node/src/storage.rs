#![allow(clippy::upper_case_acronyms)]

mod mempool;
mod smt;
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
    time::Instant,
};

use self::mempool::Mempool;

#[cfg(feature = "metrics")]
use crate::prometheus::{AWS_INSTANCE_ID, AWS_REGION};

use std::time::Duration;

use anyhow::Context;
use arc_swap::ArcSwap;
use dashmap::DashMap;
use futures_util::Stream;
use parking_lot::RwLock;
use smol::{channel::Sender, prelude::*};
use smol_timeout::TimeoutExt;
pub use smt::*;

use stdcode::StdcodeSerializeExt;
use themelio_nodeprot::TrustStore;
use themelio_stf::{CoinMapping, GenesisConfig, SealedState, SmtMapping, State};
use themelio_structs::{Block, BlockHeight, ConsensusProof, Header, NetID, ProposerAction};

#[derive(Clone)]
pub struct NodeTrustStore(pub NodeStorage);

impl TrustStore for NodeTrustStore {
    fn set(&self, netid: NetID, trusted: themelio_nodeprot::TrustedHeight) {
        self.0
            .metadata
            .insert(
                stdcode::serialize(&netid).expect("cannot serialize netid"),
                stdcode::serialize(&(trusted.height, trusted.header_hash))
                    .expect("Cannot serialize trusted height"),
            )
            .expect("could not set trusted height");
    }

    fn get(&self, netid: NetID) -> Option<themelio_nodeprot::TrustedHeight> {
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

/// NodeStorage encapsulates all storage used by a Themelio full node (auditor or staker).
#[derive(Clone)]
pub struct NodeStorage {
    mempool: Arc<RwLock<Mempool>>,
    metadata: boringdb::Dict,
    highest: Arc<ArcSwap<SealedState<MeshaCas>>>,
    old_cache: Arc<DashMap<BlockHeight, SealedState<MeshaCas>>>,
    forest: Arc<novasmt::Database<MeshaCas>>,
    _death: Sender<()>,
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

    /// Opens a NodeStorage, given a meshanina and boringdb database.
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
            .map(|b| {
                log::warn!("ACTUALLY LOADING DB");
                SealedState::from_partial_encoding_infallible(&b, &forest)
            })
            .unwrap_or_else(|| genesis.realize(&forest).seal(None));
        log::info!("HIGHEST AT {}", highest.inner_ref().height);
        let (send, recv) = smol::channel::bounded(1);
        let r = Self {
            mempool: Arc::new(Mempool::new(highest.next_state()).into()),
            highest: ArcSwap::new(Arc::new(highest)).into(),
            forest: forest.into(),
            old_cache: Default::default(),
            metadata: metadata.clone(),
            _death: send,
        };
        let highest = r.highest.clone();
        let forest = r.forest.clone();
        smolscale::spawn(async move {
            let mut dead = false;
            while !dead {
                if recv.recv().timeout(Duration::from_secs(5)).await.is_some() {
                    log::warn!("syncer dying");
                    dead = true;
                };
                let start = Instant::now();
                let highest = highest.load_full();
                let forest = forest.clone();
                smol::unblock(move || forest.storage().flush()).await;
                metadata
                    .insert(b"last_confirmed".to_vec(), highest.partial_encoding())
                    .unwrap();
                let elapsed = start.elapsed();
                if elapsed.as_secs() > 5 {
                    log::warn!("**** FLUSHED IN {:?} ****", elapsed);
                }
            }
        })
        .detach();
        r
    }

    /// Restores from a backup. Requires *exclusive* access to the storage, so do this before sharing the storage.
    pub async fn restore_pruned<S: Stream<Item = String> + Unpin>(
        &mut self,
        mut backup: S,
    ) -> anyhow::Result<()> {
        defmac::defmac!(read_tree => {
            let mut empty_tree = self.forest().get_tree(Default::default()).unwrap();
            async {
                let count: u64 = backup.next().await.context("cannot read count")?.parse()?;
                for _ in 0..count {
                    let line = backup.next().await.context("cannot read tree element")?;
                    let mut splitted = line.split(';');
                    let key_base64 = splitted.next().context("no first half")?;
                    let value_base64 = splitted.next().context("no first half")?;
                    let key: [u8; 32] = base64::decode(key_base64)?
                        .try_into()
                        .ok()
                        .context("key not 32 bytes")?;
                    let value = base64::decode(value_base64)?;
                    empty_tree.insert(key, &value);
                }
                Ok::<_, anyhow::Error>(empty_tree)
            }
        });
        let header: Header = stdcode::deserialize(&base64::decode(
            &backup.next().await.context("cannot read header")?,
        )?)?;
        let prop_action: Option<ProposerAction> = stdcode::deserialize(&base64::decode(
            &backup.next().await.context("cannot read prop action")?,
        )?)?;
        let history = read_tree!().await?;
        let coins = read_tree!().await?;
        let transactions = read_tree!().await?;
        let pools = read_tree!().await?;
        let stakes = read_tree!().await?;
        let new_state = State {
            network: header.network,
            height: header.height,
            history: SmtMapping::new(history),
            coins: CoinMapping::new(coins),
            transactions: SmtMapping::new(transactions),
            fee_pool: header.fee_pool,
            fee_multiplier: header.fee_multiplier,
            tips: Default::default(),
            dosc_speed: header.dosc_speed,
            pools: SmtMapping::new(pools),
            stakes: SmtMapping::new(stakes),
        };
        let new_highest = SealedState::from_parts(new_state, prop_action);
        self.highest.store(Arc::new(new_highest));
        let block_count: u64 = backup.next().await.context("cannot read count")?.parse()?;
        for i in 0..block_count {
            log::info!("additional block {}", i);
            let block: Block = stdcode::deserialize(&base64::decode(
                &backup.next().await.context("cannot read block")?,
            )?)?;
            let cproof: ConsensusProof = stdcode::deserialize(&base64::decode(
                &backup.next().await.context("cannot read cproof")?,
            )?)?;
            self.apply_block(block, cproof).await?;
        }
        smol::Timer::after(Duration::from_secs(3)).await;
        Ok(())
    }

    /// Serializes the storage in a pruned, textual form that discards history.
    pub fn backup_pruned(&self) -> impl Stream<Item = String> {
        let (send, recv) = smol::channel::bounded::<String>(1);
        let this = self.clone();
        smolscale::spawn(async move {
            let send_tree = {
                let send = &send;
                move |tree: novasmt::Tree<MeshaCas>| async move {
                    log::info!("** backing up tree with {} elements **", tree.count());
                    send.send(format!("{}", tree.count())).await?;
                    let count = tree.count();
                    let start = Instant::now();
                    for (i, (k, v)) in tree.iter().enumerate() {
                        let s = format!(
                            "{};{}",
                            base64::encode_config(&k, base64::STANDARD_NO_PAD),
                            base64::encode_config(&v, base64::STANDARD_NO_PAD)
                        );
                        send.send(s).await?;
                        if i as u64 % (count / 1000).max(1) == 0 {
                            log::debug!(
                                "** {}% done ({} Hz) **",
                                ((i as u64 * 1000) / count) as f64 / 10.0,
                                (i as f64) / start.elapsed().as_secs_f64()
                            );
                        }
                    }
                    Ok::<_, anyhow::Error>(())
                }
            };
            let base_state = if this.highest_height().0 <= 10000 {
                this.highest_state()
            } else {
                this.get_state(this.highest_height() - BlockHeight(10000))
                    .unwrap_or_else(|| this.highest_state())
            };
            log::info!(
                "** backup base state at height {} **",
                base_state.inner_ref().height
            );
            send.send(base64::encode_config(
                base_state.header().stdcode(),
                base64::STANDARD_NO_PAD,
            ))
            .await?;
            send.send(base64::encode_config(
                base_state.proposer_action().stdcode(),
                base64::STANDARD_NO_PAD,
            ))
            .await?;
            for tree in [
                base_state.inner_ref().history.mapping.clone(),
                base_state.inner_ref().coins.inner().clone(),
                base_state.inner_ref().transactions.mapping.clone(),
                base_state.inner_ref().pools.mapping.clone(),
                base_state.inner_ref().stakes.mapping.clone(),
            ] {
                send_tree(tree).await?;
            }
            // then for every state up to the highest state, we send the whole block
            let highest = this.highest_height();
            let count = (base_state.inner_ref().height.0..=highest.0)
                .skip(1)
                .count();
            log::info!("total number of blocks {}", count);
            send.send(format!("{}", count)).await?;
            for later_height in (base_state.inner_ref().height.0..=highest.0).skip(1) {
                log::info!("** backing up subsequent block {} **", later_height);
                let block = this
                    .get_state(later_height.into())
                    .expect("cannot get older state while backing up")
                    .to_block();
                let cproof = this
                    .get_consensus(later_height.into())
                    .expect("cannot get older cproof while backing up");
                send.send(base64::encode_config(
                    &block.stdcode(),
                    base64::STANDARD_NO_PAD,
                ))
                .await?;
                send.send(base64::encode_config(
                    &cproof.stdcode(),
                    base64::STANDARD_NO_PAD,
                ))
                .await?;
            }
            Ok::<_, anyhow::Error>(())
        })
        .detach();
        recv
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
        let highest = self.highest_state();
        if height == highest.inner_ref().height {
            return Some(highest);
        }
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
            .map(|r| {
                assert_eq!(r.inner_ref().height, height);
                r.clone()
            })
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
        // TODO!!!! CHECK INTEGRITY?!!?!?!!
        let new_state = smol::unblock(move || highest_state.apply_block(&blk)).await?;
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
        log::debug!("applied block {}", header.height);
        #[cfg(feature = "metrics")]
        log::debug!(
            "hostname={} public_ip={} network={} region={} instance_id={} applied block {}",
            crate::prometheus::HOSTNAME.as_str(),
            crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
            crate::prometheus::NETWORK
                .read()
                .expect("Could not get a read lock on NETWORK."),
            AWS_REGION.read().expect("Could not get a read lock on AWS_REGION"),
            AWS_INSTANCE_ID.read().expect("Could not get a read lock on AWS_INSTANCE_ID"),
            header.height
        );
        let next = self.highest_state().next_state();
        self.mempool_mut().rebase(next);
        Ok(())
    }

    /// Gets the forest.
    pub fn forest(&self) -> &novasmt::Database<MeshaCas> {
        &self.forest
    }
}