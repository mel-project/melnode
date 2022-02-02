use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use novasmt::ContentAddrStore;
use num_integer::Integer;
use stdcode::StdcodeSerializeExt;
use themelio_stf::SealedState;
use themelio_structs::{Block, BlockHeight};
use thiserror::Error;
use tmelcrypt::Hashable;
use tmelcrypt::{Ed25519PK, HashVal};

use crate::msg::{ProposalSig, VoteSig};

pub struct BlockGraph<C: ContentAddrStore> {
    root: SealedState<C>,
    parent_to_child: BTreeMap<HashVal, BTreeSet<HashVal>>,
    proposals: BTreeMap<HashVal, Proposal>,
    votes: BTreeMap<HashVal, BTreeMap<Ed25519PK, VoteSig>>,
    vote_weights: BTreeMap<Ed25519PK, u128>,
    correct_proposer: Box<dyn Fn(BlockHeight) -> Ed25519PK + Send + Sync + 'static>,
}

impl<C: ContentAddrStore> BlockGraph<C> {
    /// Returns whether a node has the right number of votes.
    fn is_notarized(&self, hash: HashVal) -> bool {
        if let Some(votes) = self.votes.get(&hash) {
            let total_voting_for: u128 = votes.keys().map(|k| self.vote_weights[k]).sum();
            let total_stake: u128 = self.vote_weights.values().copied().sum();
            total_voting_for > (total_stake * 2).div_ceil(&3)
        } else {
            false
        }
    }

    /// Gets the state at a given hash
    fn get_state(&self, hash: HashVal) -> Option<SealedState<C>> {
        if hash == self.root.header().hash() {
            Some(self.root.clone())
        } else {
            let prop = self.proposals.get(&hash).cloned()?;
            let mut previous = self
                .get_state(prop.extends_from)
                .expect("dangling pointer within block graph");
            while previous.inner_ref().height + BlockHeight(1) < prop.block.header.height {
                previous = previous.next_state().seal(None);
            }
            Some(
                previous
                    .apply_block(&prop.block)
                    .expect("invalid blocks inside the block graph"),
            )
        }
    }

    /// Drains out finalized blocks.
    pub fn drain_finalized(&mut self) -> Vec<SealedState<C>> {
        // DFS through the whole thing, keeping track of how many consecutively increasing notarized blocks we see
        let mut dfs_stack: Vec<(HashVal, BlockHeight, usize)> =
            vec![(self.root.header().hash(), self.root.inner_ref().height, 1)];
        while let Some((fringe_node, height, consec)) = dfs_stack.pop() {
            if consec >= 3 {
                let finalized_tip = self.proposals[&fringe_node].extends_from;
                let mut finalized_props = vec![self.proposals[&finalized_tip].clone()];
                while let Some(previous) = finalized_props
                    .last()
                    .and_then(|b| self.proposals.get(&b.extends_from))
                    .cloned()
                {
                    finalized_props.push(previous);
                }
                log::debug!("got {} finalized proposals", finalized_props.len());
                finalized_props.reverse();
                let mut accum: Vec<SealedState<C>> = vec![];
                for prop in finalized_props {
                    while accum
                        .last()
                        .map(|last| last.header().hash() != prop.block.header.previous)
                        .unwrap_or(false)
                    {
                        accum.push(accum.last().unwrap().next_state().seal(None));
                    }
                    accum.push(
                        accum
                            .last()
                            .cloned()
                            .unwrap_or_else(|| self.root.clone())
                            .apply_block(&prop.block)
                            .expect("finalized some bad blocks"),
                    );
                }
                return accum;
            }
            for child in self.parent_to_child[&fringe_node].iter().copied() {
                let actual_child = self.proposals[&child].clone();
                let child_height = actual_child.block.header.height;
                if child_height == height + BlockHeight(1) {
                    dfs_stack.push((child, child_height, consec + 1))
                } else {
                    dfs_stack.push((child, child_height, 1))
                }
            }
        }
        vec![]
    }

    /// Inserts a proposal to the block graph. If it fails, returns exactly why the proposal failed.
    pub fn insert_proposal(&mut self, prop: Proposal) -> Result<(), ProposalRejection> {
        let mut previous = match self.get_state(prop.extends_from) {
            Some(s) => s,
            None => return Err(ProposalRejection::NoPrevious(prop.extends_from)),
        };
        if previous.inner_ref().height >= prop.block.header.height {
            return Err(ProposalRejection::InvalidBlock(anyhow::anyhow!(
                "previous block at the same or higher height"
            )));
        }
        while previous.inner_ref().height + BlockHeight(1) < prop.block.header.height {
            previous = previous.next_state().seal(None);
        }
        if let Err(err) = previous.apply_block(&prop.block) {
            return Err(ProposalRejection::InvalidBlock(err.into()));
        }
        self.parent_to_child
            .entry(prop.extends_from)
            .or_default()
            .insert(prop.block.header.hash());
        self.proposals.insert(prop.block.header.hash(), prop);
        // TODO check for turn info
        Ok(())
    }

    /// Insert a vote to the block graph.
    pub fn insert_vote(&mut self, vote_for: HashVal, voter: Ed25519PK, vote: VoteSig) {
        if vote.verify(voter, vote_for) && self.proposals.contains_key(&vote_for) {
            self.votes.entry(vote_for).or_default().insert(voter, vote);
        }
    }

    /// Create a summary of this block graph to compare with somebody else's block graph.
    pub fn summarize(&self) -> BTreeMap<HashVal, HashVal> {
        self.proposals
            .iter()
            .map(|(k, _)| {
                let other = self
                    .votes
                    .get(k)
                    .cloned()
                    .unwrap_or_default()
                    .stdcode()
                    .hash();
                (*k, other)
            })
            .collect()
    }

    /// Create a PARTIAL diff between this block graph and the given summary
    pub fn diff(&self, their_summary: &BTreeMap<HashVal, HashVal>) -> Vec<BlockGraphDiff> {
        // Votes on blocks they have are more important, so we add them first
        let mut toret = Vec::new();
        for (k, v) in their_summary.iter() {
            if let Some(our_votes) = self.votes.get(k) {
                if our_votes.stdcode().hash() != *v {
                    for (pk, vote) in our_votes {
                        toret.push(BlockGraphDiff::Vote(*k, *pk, vote.clone()));
                    }
                }
            }
        }
        // return early now if we got votes
        if !toret.is_empty() {
            return toret;
        }
        // find proposals that 1. they don't have 2. would be accepted by them because they extend from things that they do have
        for (hash, prop) in self.proposals.iter() {
            if !their_summary.contains_key(hash) && their_summary.contains_key(&prop.extends_from) {
                toret.push(BlockGraphDiff::Proposal(prop.clone()));
                break;
            }
        }
        toret
    }
}

/// A diff
pub enum BlockGraphDiff {
    Proposal(Proposal),
    Vote(HashVal, Ed25519PK, VoteSig),
}

/// Why a proposal might be rejected.
#[derive(Error, Debug)]
pub enum ProposalRejection {
    #[error("proposer proposed when it's not their turn")]
    WrongTurn,
    #[error("invalid block ({0:?})")]
    InvalidBlock(anyhow::Error),
    #[error("missing extends_from")]
    NoPrevious(HashVal),
}

pub enum Node<C: ContentAddrStore> {
    Block(Arc<SealedState<C>>, Ed25519PK, ProposalSig),
    Vote(HashVal, VoteSig),
}

#[derive(Clone)]
pub struct Proposal {
    extends_from: HashVal,
    block: Block,
    proposer: Ed25519PK,
    signature: ProposalSig,
}