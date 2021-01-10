use blkstructs::FinalizedState;
use tmelcrypt::{Ed25519PK, HashVal};

use crate::msg;

/// Type that encapsulates the state of a Streamlet instance.
#[derive(Clone)]
pub struct ChainState {
    // the main database
    blocks: im::HashMap<HashVal, CsBlock>,
    // chain tips
    tips: im::HashSet<HashVal>,
    // stake mapping
    stakes: blkstructs::StakeMapping,
    // epoch
    epoch: u64,
}

impl ChainState {
    /// Create a new ChainState based on a given known last good block
    pub fn new(genesis: FinalizedState, stakes: blkstructs::StakeMapping, epoch: u64) -> Self {
        let genhash = genesis.header().hash();
        let mut blocks = im::HashMap::new();
        blocks.insert(
            genesis.header().hash(),
            CsBlock {
                state: genesis,
                votes: im::HashSet::new(),
            },
        );
        Self {
            blocks,
            tips: im::hashset![genhash],
            stakes,
            epoch,
        }
    }

    /// Find the tip of the longest notarized chain
    pub fn get_lnc_tip(&self) -> HashVal {
        *self
            .notarized_tips()
            .iter()
            .max_by(
                |&&a, &&b| match self.get_weight(a).cmp(&self.get_weight(b)) {
                    std::cmp::Ordering::Equal => a.cmp(&b),
                    o => o,
                },
            )
            .expect("there must be a LNC at all times")
    }

    /// Get a block
    pub(crate) fn get_block(&self, hash: HashVal) -> anyhow::Result<&CsBlock> {
        self.blocks
            .get(&hash)
            .ok_or_else(|| anyhow::anyhow!("nonexistent block"))
    }

    /// Get a block mutably
    fn get_block_mut(&mut self, hash: HashVal) -> anyhow::Result<&mut CsBlock> {
        self.blocks
            .get_mut(&hash)
            .ok_or_else(|| anyhow::anyhow!("nonexistent block"))
    }

    /// Process a vote.
    pub fn process_vote(&mut self, voter: Ed25519PK, vote_for: HashVal) -> anyhow::Result<()> {
        self.get_block_mut(vote_for)?.votes.insert(voter);
        Ok(())
    }

    /// Process a proposal.
    pub fn process_proposal(&mut self, prop: msg::ActualProposal) -> anyhow::Result<()> {
        let mut this = self.clone();

        // check that the proposal extends the LNC
        if prop.extends_from() != this.get_lnc_tip() {
            anyhow::bail!("proposal does not extend the LNC");
        }
        let mut prop_ancestor = this.get_block(prop.extends_from())?.state.clone();
        if prop.height() <= prop_ancestor.header().height {
            anyhow::bail!("proposal can't have smaller height than the ancestor");
        }
        // add "prosthetic" empty blocks
        while prop_ancestor.header().height + 1 < prop.height() {
            prop_ancestor = prop_ancestor.next_state().finalize();
            this.insert_block(CsBlock {
                state: prop_ancestor.clone(),
                votes: Default::default(),
            });
        }
        // process the actual block
        let mut state = prop_ancestor.next_state();
        state.apply_tx_batch(&prop.block.transactions)?;
        let state = state.finalize();
        if state.header() != prop.block.header {
            anyhow::bail!("header mismatch after applying transactions to parent");
        }
        this.insert_block(CsBlock {
            state,
            votes: Default::default(),
        });
        *self = this;
        Ok(())
    }

    /// Insert a block
    fn insert_block(&mut self, block: CsBlock) {
        let header = block.state.header();
        let tip_removed = self.tips.remove(&header.previous);
        self.blocks.insert(header.hash(), block);
        if tip_removed.is_some() {
            self.tips.insert(header.hash());
        }
    }

    /// Length of the chain, minus empty blocks, minus some pruning constant
    fn get_weight(&self, hash: HashVal) -> u64 {
        match self.blocks.get(&hash) {
            None => 0,
            Some(csb) => {
                let curr = if csb.is_empty() { 0 } else { 1 };
                curr + self.get_weight(csb.parent())
            }
        }
    }

    /// Notarized tips
    fn notarized_tips(&self) -> Vec<HashVal> {
        // if there's only one, that's it
        if self.blocks.len() == 1 {
            return vec![*self.blocks.keys().next().unwrap()];
        }
        self.tips
            .iter()
            .filter(|v| {
                let blk = self.blocks.get(v).unwrap();
                blk.votes
                    .iter()
                    .map(|k| self.stakes.vote_power(self.epoch, *k))
                    .sum::<f64>()
                    > 0.7
            })
            .cloned()
            .collect()
    }
}

/// An individual entry in the chainstate
#[derive(Clone)]
pub(crate) struct CsBlock {
    pub state: FinalizedState,
    votes: im::HashSet<Ed25519PK>,
}

impl CsBlock {
    fn is_empty(&self) -> bool {
        self.state.inner_ref().transactions.root_hash() == HashVal::default()
    }

    fn parent(&self) -> HashVal {
        self.state.header().previous
    }
}
