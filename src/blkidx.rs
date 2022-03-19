use std::{sync::Arc, time::Instant};

use dashmap::DashMap;
use std::time::Duration;
use themelio_structs::{Address, Block, BlockHeight, CoinID, TxKind};

use super::NodeStorage;

/// Indexes blocks (by pulling them out of storage)
pub struct BlockIndexer {
    height_to_map: Arc<DashMap<BlockHeight, CoinIndex>>,
    _task: smol::Task<()>,
}

impl BlockIndexer {
    /// Creates a new block indexer that pulls blocks out of the given storage, asynchronously.
    pub fn new(storage: NodeStorage) -> Self {
        let height_to_map: Arc<DashMap<BlockHeight, CoinIndex>> = Default::default();
        let h2m = height_to_map.clone();
        let _task = smolscale::spawn(async move {
            let mut next_unindexed = BlockHeight(1);
            let mut last_run = Instant::now();
            loop {
                if storage.highest_height() >= next_unindexed {
                    let top = storage.highest_height().0;
                    for height in next_unindexed.0..=top {
                        let frac = height as f64 / top as f64;
                        if last_run.elapsed().as_secs_f64() > 0.25 {
                            log::debug!("INDEXING block {} ({:.2}%)", height, frac * 100.0);
                            last_run = Instant::now();
                        }
                        let height = BlockHeight(height);
                        let state = storage.get_state(height).expect("gap in blocks?!");
                        let apply_onto = h2m
                            .get(&BlockHeight(height.0.saturating_sub(1)))
                            .map(|r| r.value().clone())
                            .unwrap_or_default();
                        let new = apply_onto.process_block(&state.to_block());
                        h2m.insert(height, new);
                        if height > BlockHeight(10000) {
                            h2m.remove(&(height - BlockHeight(10000)));
                        }
                        smol::future::yield_now().await;
                        next_unindexed = BlockHeight(height.0 + 1);
                    }
                }
                smol::Timer::after(Duration::from_secs(1)).await;
            }
        });
        Self {
            _task,
            height_to_map,
        }
    }

    /// Gets out a particular height.
    pub fn get(&self, height: BlockHeight) -> Option<CoinIndex> {
        self.height_to_map.get(&height).map(|r| r.value().clone())
    }
}

#[derive(Clone, Default)]
pub struct CoinIndex {
    owner_to_coins: imbl::HashMap<Address, imbl::HashSet<CoinID>>,
    coin_to_owner: imbl::HashMap<CoinID, Address>,
}

impl CoinIndex {
    /// Process a whole block.
    pub fn process_block(mut self, blk: &Block) -> Self {
        // add the outputs
        for tx in blk.transactions.iter() {
            for (i, output) in tx.outputs.iter().enumerate() {
                self.add_coin(tx.output_coinid(i as u8), output.covhash);
            }
        }
        // remove the inputs
        for tx in blk.transactions.iter() {
            for input in tx.inputs.iter() {
                self.remove_coin(*input)
            }
        }
        // liquidity deposit problems
        for tx in blk
            .transactions
            .iter()
            .filter(|tx| tx.kind == TxKind::LiqDeposit)
        {
            // if both are still unspent, we delete the second
            if self.coin_to_owner.contains_key(&tx.output_coinid(0))
                && self.coin_to_owner.contains_key(&tx.output_coinid(1))
            {
                self.remove_coin(tx.output_coinid(1))
            }
        }
        // liquidity withdrawal problems
        for tx in blk
            .transactions
            .iter()
            .filter(|tx| tx.kind == TxKind::LiqWithdraw)
        {
            // if both are still unspent, we delete the second
            if self.coin_to_owner.contains_key(&tx.output_coinid(0)) {
                self.add_coin(tx.output_coinid(1), tx.outputs[0].covhash);
            }
        }
        // add the proposer action
        if let Some(action) = blk.proposer_action {
            let reward_addr = action.reward_dest;
            let pseudo_coin = CoinID::proposer_reward(blk.header.height);
            self.add_coin(pseudo_coin, reward_addr)
        }
        self
    }

    /// Look up coins
    pub fn lookup(&self, owner: Address) -> Vec<CoinID> {
        self.owner_to_coins
            .get(&owner)
            .map(|e| e.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn add_coin(&mut self, id: CoinID, addr: Address) {
        self.coin_to_owner.insert(id, addr);
        self.owner_to_coins.entry(addr).or_default().insert(id);
    }

    fn remove_coin(&mut self, id: CoinID) {
        if let Some(existing) = self.coin_to_owner.remove(&id) {
            self.owner_to_coins.entry(existing).or_default().remove(&id);
        }
    }
}
