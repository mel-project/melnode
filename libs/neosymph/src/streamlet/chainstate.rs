use blkstructs::SealedState;
use tmelcrypt::{Ed25519PK, HashVal};

use crate::{msg, OOB_PROPOSER_ACTION};

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
    pub fn new(genesis: SealedState, stakes: blkstructs::StakeMapping, epoch: u64) -> Self {
        let genhash = genesis.header().hash();
        let mut blocks = im::HashMap::new();
        blocks.insert(
            genesis.header().hash(),
            CsBlock {
                state: genesis,
                votes: im::HashSet::new(),
                vote_weight: 1.0,
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
        let vote_power = self.stakes.vote_power(self.epoch, voter);
        let blk = self.get_block_mut(vote_for)?;
        if blk.votes.insert(voter).is_none() {
            blk.vote_weight += vote_power;
        }
        log::debug!(
            "added vote from {:?}; now {} votes for {:?}",
            voter,
            blk.vote_weight,
            blk.state.header().hash()
        );
        Ok(())
    }

    /// Forcibly sets a block to final
    pub fn force_finalize(&mut self, state: SealedState) -> anyhow::Result<()> {
        log::warn!("force finalizing {:?}", state.header().hash());
        let synth = CsBlock {
            state,
            vote_weight: 1.0,
            votes: Default::default(),
        };
        self.insert_block(synth);
        Ok(())
    }

    /// The last block of the epoch
    fn last_block_in_epoch(&self) -> u64 {
        (self.epoch + 1) * blkstructs::STAKE_EPOCH - 1
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
            prop_ancestor = prop_ancestor.next_state().seal(None);
            this.insert_block(CsBlock {
                state: prop_ancestor.clone(),
                votes: Default::default(),
                vote_weight: 0.0,
            });
        }
        // process the actual block
        if prop.height() > self.last_block_in_epoch()
            && (!prop.block.transactions.is_empty()
                || prop.block.proposer_action != Some(OOB_PROPOSER_ACTION))
        {
            anyhow::bail!("proposal is out of epoch bounds yet it isn't totally empty")
        }
        let state = prop_ancestor.apply_block(&prop.block)?;
        this.insert_block(CsBlock {
            state,
            votes: Default::default(),
            vote_weight: 0.0,
        });
        *self = this;
        Ok(())
    }

    /// Produces a string representing a GraphViz visualization of the block state.
    pub fn graphviz(&self) -> String {
        let finalized: im::HashSet<HashVal> = self
            .clone()
            .drain_finalized()
            .into_iter()
            .map(|v| v.header().hash())
            .collect();
        let mut buff = String::new();
        buff.push_str("digraph G {\n");

        for (node_id, node) in self.blocks.iter() {
            buff.push_str(&format!(
                "\"{:?}\" [shape=rectangle, label=\"{:?} ({}%, w={})\", style=filled, {}];\n",
                node_id,
                node_id,
                (node.vote_weight * 100.0) as i32,
                self.get_weight(*node_id),
                if self.get_lnc_tip() == *node_id {
                    "fillcolor=aquamarine"
                } else if finalized.contains(node_id) {
                    "fillcolor=darkseagreen1"
                } else if self.tips.contains(&node_id) {
                    "fillcolor=azure"
                } else {
                    ""
                }
            ));
            let parent_id = node.parent();
            buff.push_str(&format!("\"{:?}\" -> \"{:?}\"\n", node_id, parent_id));
        }

        buff.push('}');
        buff
    }

    /// Drain all finalized blocks out of the system. Prunes the chainstate to start from the last finalized block, discarding absolutely everything else.
    pub fn drain_finalized(&mut self) -> Vec<SealedState> {
        // first we find the last finalized block
        let mut toret = Vec::new();
        if let Some(mut tip) = self.find_finalized() {
            while let Some(blk) = self.blocks.get(&tip) {
                toret.push(blk.clone());
                tip = blk.parent();
            }
        }
        toret.reverse();
        // then we "reset"
        if let Some(finalized_tip) = toret.last() {
            let finalized_hash = finalized_tip.state.header().hash();
            let mut new_self =
                Self::new(finalized_tip.state.clone(), self.stakes.clone(), self.epoch);
            // copy all branches that root at the new tip
            for &tip in self.tips.iter() {
                let branch = self.get_branch(tip);
                if branch
                    .iter()
                    .any(|v| v.state.header().hash() == finalized_hash)
                {
                    for elem in branch {
                        if elem.state.header().hash() == finalized_hash {
                            break;
                        }
                        new_self.insert_block(elem.clone())
                    }
                }
            }
            // we also insert the parent of the last finalized block into the new self.
            // this way, we can immediately finalize the next block because there will be a "three-in-a-row".
            if let Some(penultimate) = toret.get(toret.len() - 2) {
                new_self.insert_block(penultimate.clone());
            }
            *self = new_self;
        }
        toret.into_iter().map(|v| v.state).collect()
    }

    fn find_finalized(&self) -> Option<HashVal> {
        self.notarized_tips()
            .iter()
            .filter_map(|v| self.finalized_portion(*v))
            .max_by_key(|v| v.header().height)
            .map(|v| v.header().hash())
    }

    fn get_branch(&self, tip: HashVal) -> Vec<&CsBlock> {
        let mut tip = tip;
        let mut accum = Vec::new();
        while let Some(blk) = self.blocks.get(&tip) {
            accum.push(blk);
            tip = blk.parent();
        }
        accum
    }

    fn finalized_portion(&self, tip: HashVal) -> Option<&SealedState> {
        // "linearize" into a vector
        let full_branch = {
            let mut tip = tip;
            let mut accum = Vec::new();
            while let Some(blk) = self.blocks.get(&tip) {
                accum.push(&blk.state);
                tip = blk.parent();
            }
            accum
        };
        // find three consecutive non-empty blocks
        for idx in 1..full_branch.len() - 1 {
            if !full_branch[idx - 1].is_empty()
                && !full_branch[idx].is_empty()
                && !full_branch[idx + 1].is_empty()
            {
                return Some(full_branch[idx]);
            }
        }
        None
    }

    /// Insert a block
    fn insert_block(&mut self, block: CsBlock) {
        let header = block.state.header();
        self.tips.remove(&header.previous);
        self.blocks.insert(header.hash(), block);
        self.tips.insert(header.hash());
    }

    /// Length of the chain, minus empty blocks, minus some pruning constant
    fn get_weight(&self, hash: HashVal) -> u64 {
        match self.blocks.get(&hash) {
            None => 0,
            Some(csb) => {
                let curr = if csb.state.is_empty() { 0 } else { 1 };
                curr + self.get_weight(csb.parent())
            }
        }
    }

    /// Notarized tips
    fn notarized_tips(&self) -> im::HashSet<HashVal> {
        self.tips
            .iter()
            .cloned()
            .filter_map(|mut v| {
                while let Some(blk) = self.blocks.get(&v) {
                    if blk.is_notarized() {
                        return Some(v);
                    }
                    v = blk.parent();
                }
                None
            })
            .collect()
    }
}

/// An individual entry in the chainstate
#[derive(Clone, Debug)]
pub(crate) struct CsBlock {
    pub state: SealedState,
    votes: im::HashSet<Ed25519PK>,
    vote_weight: f64,
}

impl CsBlock {
    fn is_notarized(&self) -> bool {
        self.vote_weight >= 0.7
    }

    fn parent(&self) -> HashVal {
        self.state.header().previous
    }
}
