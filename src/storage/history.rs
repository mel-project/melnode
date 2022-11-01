use std::{
    io::{ErrorKind, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use crossbeam_queue::SegQueue;

use tap::Tap;
use themelio_structs::{Block, BlockHeight, ConsensusProof};

/// History stores the history of the blockchain in a simple, flat-file format. The format is super simple, a directory with:
/// - a file `highest` storing the height of the highest block
/// - 10-digit-long files storing the blocks, named after the block height (e.g. `0000000001.blk` stores block 1)
pub struct History {
    base_path: PathBuf,
    dirty: SegQueue<u64>,
    highest: AtomicU64,
}

impl History {
    /// Creates a new History.
    pub fn new(base_path: PathBuf) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&base_path)?;
        Ok(Self {
            base_path,
            dirty: Default::default(),
            highest: 0.into(),
        })
    }

    /// Reads the highest block height.
    pub fn highest(&self) -> BlockHeight {
        self.highest.load(Ordering::SeqCst).into()
    }

    /// Reads a particular block from disk. Returns None if that block is unavailable.
    ///
    /// Due to pruning, etc, it's not always guaranteed that every block below the highest block is available.
    pub fn get_block(
        &self,
        height: BlockHeight,
    ) -> anyhow::Result<Option<(Block, ConsensusProof)>> {
        let path = self
            .base_path
            .clone()
            .tap_mut(|p| p.push(format!("{:0>9}.blk", height.0)));
        match std::fs::read(&path) {
            Ok(b) => Ok(Some(stdcode::deserialize(&b)?)),
            Err(err) => {
                if err.kind() == ErrorKind::NotFound {
                    Ok(None)
                } else {
                    Err(err.into())
                }
            }
        }
    }

    /// Writes a particular block to disk. Does NOT update the on-disk record of what the highest block is, which is only written on [History::flush].
    pub fn insert_block(&self, block: &Block, proof: &ConsensusProof) -> anyhow::Result<()> {
        let path = self
            .base_path
            .clone()
            .tap_mut(|p| p.push(format!("{:0>9}.blk", block.header.height.0)));
        std::fs::write(&path, stdcode::serialize(&(block, proof))?)?;
        // we dirty this block height, then set the in-memory highest pointer, which we wanna flush later.
        self.dirty.push(block.header.height.0);
        self.highest
            .fetch_max(block.header.height.0, Ordering::SeqCst);
        Ok(())
    }

    /// Flushes all information to disk. After this call, all contents are durable on disk. Importantly, the on-disk record of the highest block is **only** updated in this method.
    pub fn flush(&self) -> anyhow::Result<()> {
        let real_highest = self.highest.load(Ordering::SeqCst);
        // TODO on systems like Linux, we can call one single syscall to sync literally everything to disk. That *might* be faster.
        while let Some(dirty) = self.dirty.pop() {
            log::debug!("syncing dirty history entry {dirty}");
            let path = self
                .base_path
                .clone()
                .tap_mut(|p| p.push(format!("{:0>9}.blk", dirty)));
            std::fs::File::open(&path)?.sync_all()?;
        }
        let highest = self.base_path.clone().tap_mut(|p| p.push("highest-1"));
        let mut file = std::fs::File::create(&highest)?;
        file.write_all(real_highest.to_string().as_bytes())?;
        file.sync_all()?;
        let highest_actual = self.base_path.clone().tap_mut(|p| p.push("highest"));
        std::fs::rename(&highest, &highest_actual)?;
        std::fs::File::open(&highest_actual)?.sync_all()?;
        Ok(())
    }
}
