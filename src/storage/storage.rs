use anyhow::Context;

use event_listener::Event;
use rusqlite::{params, OptionalExtension};
use smol::channel::{Receiver, Sender};
use std::{
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::Arc,
    time::Instant,
};
use stdcode::StdcodeSerializeExt;
use tap::Tap;
use tip911_stakeset::StakeSet;

use moka::sync::Cache;
use parking_lot::RwLock;

use themelio_stf::{GenesisConfig, SealedState};
use themelio_structs::{
    Block, BlockHeight, CoinValue, ConsensusProof, NetID, StakeDoc, TxHash, TxKind,
};

use super::{mempool::Mempool, MeshaCas};

/// Storage encapsulates all storage used by a Themelio full node (auditor or staker).
#[derive(Clone)]
pub struct Storage {
    send_pool: Sender<rusqlite::Connection>,
    recv_pool: Receiver<rusqlite::Connection>,
    old_cache: Arc<Cache<BlockHeight, SealedState<MeshaCas>>>,
    forest: Arc<novasmt::Database<MeshaCas>>,

    genesis: GenesisConfig,

    mempool: Arc<RwLock<Mempool>>,

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
    pub async fn open(db_folder: PathBuf, genesis: GenesisConfig) -> anyhow::Result<Self> {
        let sqlite_path = db_folder.clone().tap_mut(|path| path.push("storage.db"));
        let mesha_path = db_folder.clone().tap_mut(|path| path.push("merkle.db"));
        let conn = rusqlite::Connection::open(&sqlite_path)?;
        conn.execute("create table if not exists history (height primary key not null, header not null, block not null)", params![])?;
        conn.execute("create table if not exists consensus_proofs (height primary key not null, proof not null)", params![])?;
        conn.execute(
            "create table if not exists stakes (txhash primary key not null, height not null, stake_doc not null)",
            params![],
        )?;
        conn.execute(
            "create table if not exists misc (key primary key not null, value not null)",
            params![],
        )?;
        let (send_pool, recv_pool) = smol::channel::unbounded();
        for _ in 0..16 {
            let conn = rusqlite::Connection::open(&sqlite_path)?;
            conn.query_row("pragma journal_mode=WAL", params![], |_| Ok(()))?;
            conn.execute("pragma synchronous=normal", params![])?;
            send_pool.send(conn).await.unwrap();
        }

        let forest = novasmt::Database::new(MeshaCas::new(meshanina::Mapping::open(&mesha_path)?));
        let mempool = Arc::new(Mempool::new(genesis.clone().realize(&forest)).into());
        Ok(Self {
            send_pool,
            recv_pool,
            old_cache: Arc::new(Cache::new(10_000)),
            forest: Arc::new(forest),

            genesis,

            new_block_notify: Arc::new(Event::new()),
            mempool,
        })
    }

    /// Obtain the highest state.
    pub async fn highest_state(&self) -> anyhow::Result<SealedState<MeshaCas>> {
        // TODO this may be a bit stale
        let height = self.highest_height().await?;
        if let Some(height) = height {
            Ok(self.get_state(height).await?.context("no highest")?)
        } else {
            Ok(self.genesis.clone().realize(self.forest()).seal(None))
        }
    }

    /// Obtain the highest height.
    pub async fn highest_height(&self) -> anyhow::Result<Option<BlockHeight>> {
        let conn = self.recv_pool.recv().await?;
        let send_pool = self.send_pool.clone();
        smol::unblock(move || {
            let conn = scopeguard::guard(conn, |conn| send_pool.try_send(conn).unwrap());
            let val: Option<u64> =
                conn.query_row("select max(height) from history", params![], |r| r.get(0))?;
            Ok(val.map(BlockHeight))
        })
        .await
    }

    /// Waits until a certain height is available, then returns it.
    pub async fn get_state_or_wait(&self, _height: BlockHeight) -> SealedState<MeshaCas> {
        todo!()
    }

    /// Reconstruct the stakeset at a given height.
    async fn get_stakeset(&self, height: BlockHeight) -> anyhow::Result<StakeSet> {
        let conn = self.recv_pool.recv().await?;
        let send_pool = self.send_pool.clone();
        let genesis = self.genesis.clone();
        smol::unblock(move || {
            let conn = scopeguard::guard(conn, |conn| send_pool.try_send(conn).unwrap());
            let mut stmt = conn.prepare("select txhash, height, stake_doc from stakes")?;
            let mut stakes = StakeSet::new(vec![].into_iter());
            // TODO this is dumb!
            for (txhash, stake) in genesis.stakes {
                stakes.add_stake(txhash, stake);
            }
            for row in
                stmt.query_map(params![], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            {
                let row: (String, u64, Vec<u8>) = row?;
                let t: TxHash = row.0.parse()?;
                let sd: StakeDoc = stdcode::deserialize(&row.2)?;
                stakes.add_stake(t, sd);
            }
            stakes.unlock_old(height.epoch());
            Ok(stakes)
        })
        .await
    }

    /// Obtain a historical SealedState.
    pub async fn get_state(
        &self,
        height: BlockHeight,
    ) -> anyhow::Result<Option<SealedState<MeshaCas>>> {
        let stakes = self.get_stakeset(height).await?;
        let conn = self.recv_pool.recv().await?;
        let send_pool = self.send_pool.clone();
        let forest = self.forest.clone();
        smol::unblock(move || {
            let conn = scopeguard::guard(conn, |conn| send_pool.try_send(conn).unwrap());
            let block_blob: Option<Vec<u8>> = conn
                .query_row(
                    "select block from history where height = $1",
                    params![height.0],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(block_blob) = block_blob {
                let block: Block = stdcode::deserialize(&block_blob)?;
                let state = SealedState::from_block(&block, &stakes, &forest);
                assert_eq!(state.header(), block.header);
                Ok(Some(state))
            } else {
                Ok(None)
            }
        })
        .await
    }

    /// Obtain a historical ConsensusProof.
    pub async fn get_consensus(
        &self,
        height: BlockHeight,
    ) -> anyhow::Result<Option<ConsensusProof>> {
        let conn = self.recv_pool.recv().await?;
        let send_pool = self.send_pool.clone();
        smol::unblock(move || {
            let conn = scopeguard::guard(conn, |conn| send_pool.try_send(conn).unwrap());
            let vec: Option<Vec<u8>> = conn
                .query_row(
                    "select proof from consensus_proofs where height = $1",
                    params![height.0],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(vec) = vec {
                Ok(Some(stdcode::deserialize(&vec)?))
            } else {
                Ok(None)
            }
        })
        .await
    }

    /// Consumes a block, applying it to the current state.
    pub async fn apply_block(&self, blk: Block, cproof: ConsensusProof) -> anyhow::Result<()> {
        let highest_state = self.highest_state().await?;
        let header = blk.header;
        if header.height != highest_state.header().height + 1.into() {
            anyhow::bail!(
                "cannot apply block {} to height {}",
                header.height,
                highest_state.header().height
            );
        }

        // Check the consensus proof
        let mut total_votes = CoinValue(0);
        let mut present_votes = CoinValue(0);
        for stake_doc_bytes in highest_state.raw_stakes().pre_tip911().iter() {
            let stake_doc: StakeDoc = stdcode::deserialize(&stake_doc_bytes.1)?;
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

        // we flush the merkle stuff first, because the sqlite points to merkle
        self.forest.storage().flush();
        // now transactionally save to sqlite
        {
            let conn = self.recv_pool.recv().await?;
            let send_pool = self.send_pool.clone();
            let _forest = self.forest.clone();
            smol::unblock(move || {
                let mut conn = scopeguard::guard(conn, |conn| send_pool.try_send(conn).unwrap());
                let conn = conn.transaction()?;
                conn.execute(
                    "insert into history (height, header, block) values ($1, $2, $3)",
                    params![blk.header.height.0, blk.header.stdcode(), blk.stdcode()],
                )?;
                for txn in blk.transactions {
                    if txn.kind == TxKind::Stake {
                        if let Ok(doc) = stdcode::deserialize::<StakeDoc>(&txn.data) {
                            // TODO BUG BUG this poorly replicates the validation logic. Make a method SealedState::new_stakes()
                            if blk.header.height.0 >= 500000 || blk.header.network != NetID::Mainnet {
                            conn.execute("insert into stakes (txhash, height, stake_doc) values ($1, $2, $3)", params![txn.hash_nosigs().to_string(), blk.header.height.0, doc.stdcode()])?;
                            }
                        }
                    }
                }
                conn.commit()?;
                anyhow::Ok(())
            })
            .await?
        }
        log::debug!(
            "applied block {} / {} in {:.2}ms (insert {:.2}ms)",
            new_state.header().height,
            new_state.header().hash(),
            apply_time.as_secs_f64() * 1000.0,
            start.elapsed().as_secs_f64() * 1000.0
        );
        let next = self.highest_state().await?;
        self.mempool_mut().rebase(next);
        self.new_block_notify.notify(usize::MAX);

        Ok(())
    }

    /// Gets the forest.
    pub fn forest(&self) -> &novasmt::Database<MeshaCas> {
        &self.forest
    }
}
