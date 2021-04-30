use std::collections::{BTreeMap, BTreeSet};
mod helpers;
use blkdb::{backends::InMemoryBackend, ApplyBlockErr, BlockTree, Cursor};
use blkstructs::{Block, StakeMapping};
use helpers::*;

mod gossip;
use gossip::*;

use tmelcrypt::{Ed25519PK, HashVal};

use crate::msg::{ProposalSig, VoteSig};

/// A representation of the chain state internal to Symphonia.
pub struct ChainState {
    epoch: u64,
    stakes: StakeMapping,
    inner: BlockTree<InMemoryBackend>,
}

impl ChainState {
    /// Process a proposal. Returns an error if the proposal is unacceptable for whatever reason.
    pub fn inject_proposal(
        &mut self,
        proposed_block: &Block,
        proposer: Ed25519PK,
        proposal_sig: ProposalSig,
        last_nonempty: HashVal,
    ) -> Result<(), ProposalError> {
        log::debug!(
            "received proposal ({}, {:?}) extending from {:?}",
            proposed_block.header.height,
            proposed_block.header.hash(),
            last_nonempty
        );
        let lnc_tips = self.get_lnc_tips();
        if !lnc_tips.contains(&last_nonempty) {
            return Err(ProposalError::NotExtendingLnc);
        }
        // check last_nonempty and fill with empty blocks
        let mut to_apply: Vec<Block> = vec![];
        {
            let last_nonempty_cursor = self
                .inner
                .get_cursor(last_nonempty)
                .expect("failed to get the extending LNC even after check");
            // if last_nonempty >= proposed height, then the height can't be right
            if last_nonempty_cursor.header().height >= proposed_block.header.height {
                return Err(ProposalError::IncorrectHeight);
            }
            // okay time to fill with empty blocks
            if last_nonempty_cursor.header().height + 1 < proposed_block.header.height {
                let empty_count =
                    proposed_block.header.height - last_nonempty_cursor.header().height - 1;
                log::debug!("filling in {} empty blocks", empty_count);
                let mut last = last_nonempty_cursor.to_state();
                for _ in 0..empty_count {
                    last = last.next_state().seal(None);
                    to_apply.push(last.to_block());
                }
                if last.apply_block(&proposed_block).is_err() {
                    return Err(ProposalError::InvalidBlock);
                }
            }
        }
        // TODO: check whether this is the *right* guy to propose this round. Not checking this potentially impacts fairness, but not correctness
        for block in to_apply {
            self.inner
                .apply_block(&block, &[])
                .expect("failed applying an empty block");
        }
        self.inner
            .apply_block(
                &proposed_block,
                &stdcode::serialize(&StreamletMetadata {
                    proposer,
                    proposal_sig,
                    votes: BTreeMap::new(),
                })
                .unwrap(),
            )
            .map_err(|_| ProposalError::InvalidBlock)?;
        Ok(())
    }

    /// Process a vote.
    pub fn inject_vote(
        &mut self,
        voting_for: HashVal,
        voter: Ed25519PK,
        signature: VoteSig,
    ) -> Result<(), VoteError> {
        let mut existing_metadata = self
            .inner
            .get_cursor(voting_for)
            .ok_or(VoteError::NoSuchBlock)?
            .get_streamlet()
            .ok_or(VoteError::EmptyBlock)?;
        existing_metadata.votes.insert(voter, signature);
        self.inner
            .get_cursor_mut(voting_for)
            .expect("failed to put metadata back in")
            .set_metadata(&stdcode::serialize(&existing_metadata).unwrap());
        Ok(())
    }

    /// Generates a block request
    pub fn new_block_request(&self) -> BlockRequest {
        BlockRequest {
            lnc_tips: self.get_lnc_tips(),
            lnc_leaves: self.get_lnc_leaves(),
        }
    }

    /// Generates a batch of block responses in response to a gossip request.
    pub fn new_block_responses(&self, request: BlockRequest) -> Vec<AbbrBlockResponse> {
        // We send over abbrblocks for all the "leaves" of *their* lnc tips, as well as all of our LNCs.
        // The idea here is that this should move their lnc tips forwards, because if any descendant of their lnc is notarized, this procedure will let them know.
        // Sending our LNCs also ensures that the other side does not miss any important data.
        // Eventually, the other side will have all the info we have, plus perhaps more.
        let their_lnc_tips = request
            .lnc_tips
            .into_iter()
            .filter_map(|v| self.inner.get_cursor(v))
            .collect::<Vec<_>>();
        let to_send = self.get_leaves(their_lnc_tips);
        to_send
            .into_iter()
            .map(|hash| {
                let cursor = self
                    .inner
                    .get_cursor(hash)
                    .expect("leaf that we just saw is now gone");
                AbbrBlockResponse {
                    abbr_block: cursor.to_state().to_block().abbreviate(),
                    metadata: cursor
                        .get_streamlet()
                        .expect("leaf cannot possibly be empty"),
                }
            })
            .collect()
    }

    /// Attempts to apply a full-block response from a gossip peer.
    pub fn apply_block_response(
        &mut self,
        response: FullBlockResponse,
    ) -> Result<(), ApplyBlockErr> {
        let mut metadata = response.metadata.clone();
        if let Some(Some(previous_metadata)) = self
            .inner
            .get_cursor(response.block.header.hash())
            .map(|v| v.get_streamlet())
        {
            for (voter, vote) in previous_metadata.votes {
                metadata.votes.insert(voter, vote);
            }
        }
        // Now we must validate the metadata to make sure it all makes sense
        let abbr_block = response.block.abbreviate();
        let mut real_metadata = BTreeMap::new();
        for (voter, vote) in metadata.votes.iter() {
            if vote.verify(*voter, &abbr_block) {
                real_metadata.insert(*voter, vote.clone());
            }
        }
        self.inner.apply_block(
            &response.block,
            &stdcode::serialize(&real_metadata).unwrap(),
        )?;
        Ok(())
    }

    /// Get LNCs
    fn get_lnc_tips(&self) -> BTreeSet<HashVal> {
        let tip_notarized_ancestors = self
            .inner
            .get_tips()
            .into_iter()
            .map(|mut tip| {
                // "move back" the tip until something notarized is found
                while !tip
                    .get_streamlet()
                    .map(|v| v.is_notarized(self.epoch, &self.stakes))
                    .unwrap_or_default()
                // empty blocks cannot be notarized in any way
                {
                    if let Some(parent) = tip.parent() {
                        tip = parent;
                    } else {
                        // Genesis is ALWAYS considered notarized
                        return tip;
                    }
                }
                tip
            })
            .collect::<Vec<_>>();
        // we filter out things that are not at the highest height
        let max_weight = tip_notarized_ancestors
            .iter()
            .map(|v| v.chain_weight())
            .max()
            .expect("no highest?!");
        tip_notarized_ancestors
            .into_iter()
            .filter(|v| v.chain_weight() == max_weight)
            .map(|v| v.header().hash())
            .collect()
    }

    /// Get LNC descendants
    fn get_lnc_leaves(&self) -> BTreeSet<HashVal> {
        let mut stack = self
            .get_lnc_tips()
            .into_iter()
            .map(|v| self.inner.get_cursor(v).unwrap())
            .collect::<Vec<_>>();
        self.get_leaves(stack)
    }

    fn get_leaves<'a>(&self, mut stack: Vec<Cursor<'a, InMemoryBackend>>) -> BTreeSet<HashVal> {
        let mut toret = BTreeSet::new();
        while let Some(top) = stack.pop() {
            for child in top.children() {
                toret.insert(child.header().hash());
                stack.push(child);
            }
            // remove if we have children
            if !top.children().is_empty() {
                toret.remove(&top.header().hash());
            }
        }

        toret
    }
}
