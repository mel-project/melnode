use std::collections::{BTreeMap, BTreeSet};
mod helpers;
use blkdb::{backends::InMemoryBackend, BlockTree, Cursor};
use blkstructs::{Block, SealedState, StakeMapping, STAKE_EPOCH};
use helpers::*;

pub mod gossip;
use gossip::*;

use tmelcrypt::{Ed25519PK, Ed25519SK, HashVal};

use crate::msg::{ProposalSig, VoteSig};

/// A representation of the chain state internal to Symphonia.
pub struct ChainState {
    epoch: u64,
    stakes: StakeMapping,
    inner: BlockTree<InMemoryBackend>,

    drained_height: u64,
}

impl ChainState {
    /// Create a new ChainState with the given genesis state.
    pub fn new(genesis: SealedState, forest: autosmt::Forest) -> Self {
        let epoch = genesis.inner_ref().height / STAKE_EPOCH;
        let stakes = genesis.inner_ref().stakes.clone();
        let mut inner = BlockTree::new(InMemoryBackend::default(), forest);
        inner.set_genesis(genesis, &[]);
        Self {
            epoch,
            stakes,
            inner,

            drained_height: 0,
        }
    }

    /// Does this block exist?
    pub fn has_block(&self, blkhash: HashVal) -> bool {
        self.inner.get_cursor(blkhash).is_some()
    }

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
        if !proposal_sig.verify(proposer, &proposed_block.abbreviate()) {
            return Err(ProposalError::InvalidBlock);
        }

        let lnc_tips = self.get_lnc_tips();
        if !lnc_tips.contains(&last_nonempty) {
            log::warn!("tips: {:?}", lnc_tips);
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
                if let Err(err) = last.apply_block(&proposed_block) {
                    log::warn!("problem applying block: {:?}", err);
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
            .map_err(|e| {
                log::warn!("error applying block: {:?}", e);
                ProposalError::InvalidBlock
            })?;
        Ok(())
    }

    /// Process a vote.
    pub fn inject_vote(
        &mut self,
        voting_for: HashVal,
        voter: Ed25519PK,
        signature: VoteSig,
    ) -> Result<(), VoteError> {
        if !signature.verify(voter, voting_for) {
            return Err(VoteError::InvalidSignature);
        }
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

    /// Votes for all "appropriate" proposals.
    pub fn vote_all(&mut self, voter_sk: Ed25519SK) {
        let lnc_cursor = self
            .get_lnc_tips()
            .into_iter()
            .next()
            .map(|v| self.inner.get_cursor(v).unwrap())
            .unwrap();
        let mut vote_for = Vec::new();
        let mut stack = lnc_cursor.children();
        while let Some(child) = stack.pop() {
            if let Some(metadata) = child.get_streamlet() {
                if metadata.votes.get(&voter_sk.to_public()).is_none() {
                    vote_for.push(child.header().hash());
                }
            } else {
                stack.extend(child.children())
            }
        }
        for hash in vote_for {
            log::debug!("self-voting for {}", hash);
            self.inject_vote(
                hash,
                voter_sk.to_public(),
                VoteSig::generate(voter_sk, hash),
            )
            .expect("vote_all should never produce an error");
        }
    }

    /// Generates a block request
    pub fn new_block_request(&self) -> BlockRequest {
        BlockRequest {
            lnc_tips: self.get_lnc_tips(),
        }
    }

    /// Generates a batch of block responses in response to a gossip request.
    pub fn new_block_responses(&self, request: BlockRequest) -> Vec<AbbrBlockResponse> {
        // We send over abbrblocks for all the descendants of *their* lnc tips
        let their_lnc_tips = request
            .lnc_tips
            .into_iter()
            .filter_map(|v| self.inner.get_cursor(v))
            .collect::<Vec<_>>();
        let to_send = self.get_nonempty_descendants(their_lnc_tips);
        to_send
            .into_iter()
            .map(|hash| {
                let cursor = self
                    .inner
                    .get_cursor(hash)
                    .expect("leaf that we just saw is now gone");
                let last_nonempty = {
                    let mut cursor = cursor.parent().expect("must have parent at this point");
                    while cursor.get_streamlet().is_none() && cursor.parent().is_some() {
                        cursor = cursor.parent().unwrap()
                    }
                    cursor.header().hash()
                };
                AbbrBlockResponse {
                    abbr_block: cursor.to_state().to_block().abbreviate(),
                    metadata: cursor.get_streamlet().unwrap(),
                    last_nonempty,
                }
            })
            .collect()
    }

    /// Generates a response to the given transaction request.
    pub fn new_transaction_response(&self, request: TransactionRequest) -> TransactionResponse {
        if let Some(cursor) = self.inner.get_cursor(request.block_hash) {
            let state = cursor.to_state();
            let mut transactions = vec![];
            for txhash in request.hashes {
                let transaction = state.inner_ref().transactions.get(&txhash).0;
                if let Some(transaction) = transaction {
                    transactions.push(transaction);
                }
            }
            TransactionResponse { transactions }
        } else {
            TransactionResponse {
                transactions: vec![],
            }
        }
    }

    /// Forcibly resets the genesis to something with the given HashVal.
    pub fn reset_genesis(&mut self, genesis: SealedState) {
        if let Some(cursor) = self.inner.get_cursor(genesis.header().hash()) {
            let state = cursor.to_state();
            let metadata = cursor.metadata().to_vec();
            drop(cursor);
            self.inner.set_genesis(state, &metadata);
        } else {
            self.inner.set_genesis(genesis, &[]);
        }
    }

    /// Attempts to apply a full-block response from a gossip peer.
    pub fn apply_block_response(&mut self, response: FullBlockResponse) -> anyhow::Result<()> {
        if self
            .inner
            .get_cursor(response.block.header.hash())
            .is_none()
        {
            self.inject_proposal(
                &response.block,
                response.metadata.proposer,
                response.metadata.proposal_sig,
                response.last_nonempty,
            )?;
        }
        let voting_for = response.block.header.hash();
        for (voter, vote) in response.metadata.votes {
            self.inject_vote(voting_for, voter, vote)?;
        }
        // let existing_metadata = self
        //     .inner
        //     .get_cursor(response.block.header.hash())
        //     .map(|v| v.get_streamlet())
        //     .flatten();
        // if let Some(mut metadata) = response.metadata.clone() {
        //     if !metadata.is_signed_correctly(&response.block.abbreviate()) {
        //         return Err(ApplyBlockErr::HeaderMismatch);
        //     }
        //     if let Some(Some(previous_metadata)) = self
        //         .inner
        //         .get_cursor(response.block.header.hash())
        //         .map(|v| v.get_streamlet())
        //     {
        //         if previous_metadata.proposer != metadata.proposer {
        //             return Err(ApplyBlockErr::HeaderMismatch);
        //         }
        //         for (voter, vote) in previous_metadata.votes {
        //             metadata.votes.insert(voter, vote);
        //         }
        //     }
        //     // Now we must validate the metadata to make sure it all makes sense
        //     let abbr_block = response.block.abbreviate();
        //     let mut real_votes = BTreeMap::new();
        //     for (voter, vote) in metadata.votes.iter() {
        //         if vote.verify(*voter, &abbr_block) {
        //             real_votes.insert(*voter, vote.clone());
        //         }
        //     }
        //     metadata.votes = real_votes;

        //     self.inner
        //         .apply_block(&response.block, &stdcode::serialize(&metadata).unwrap())?;
        // } else {
        //     self.inner.apply_block(
        //         &response.block,
        //         &(if let Some(existing_metadata) = existing_metadata {
        //             stdcode::serialize(&existing_metadata).unwrap()
        //         } else {
        //             vec![]
        //         }),
        //     )?;
        // }
        Ok(())
    }

    /// Gets an arbitrary LNC tip, fully "realized", for building the next block.
    pub fn get_lnc_state(&self) -> SealedState {
        let lowest_lnc_hash = self.get_lnc_tips().into_iter().min().unwrap();
        self.inner.get_cursor(lowest_lnc_hash).unwrap().to_state()
    }

    /// Gets the stake mapping.
    pub fn stakes(&self) -> &StakeMapping {
        &self.stakes
    }

    /// Dump the entire chainstate as a GraphViz graph.
    pub fn debug_graphviz(&self) -> String {
        let lnc_tips = self.get_lnc_tips();
        let finalized = self.get_final_tip().unwrap_or_default();
        self.inner.debug_graphviz(|cursor| {
            if cursor.header().hash() == finalized {
                "purple".into()
            } else if lnc_tips.contains(&cursor.header().hash()) {
                "green".into()
            } else if cursor.get_streamlet().is_some() {
                "blue".into()
            } else {
                "gray".into()
            }
        })
    }

    /// "Drain" all finalized blocks from the chainstate.
    pub fn drain_finalized(&mut self) -> Vec<SealedState> {
        if let Some(final_tip) = self.get_final_tip() {
            let mut finalized = self.inner.get_cursor(final_tip).unwrap();
            let new_drained_height = finalized.header().height;
            // Find all ancestors of the final tip
            let mut ancestors = vec![];
            while finalized.header().height > self.drained_height {
                ancestors.push(finalized.clone());
                if let Some(parent) = finalized.parent() {
                    finalized = parent
                } else {
                    break;
                }
            }
            ancestors.reverse();
            self.drained_height = new_drained_height;
            ancestors.into_iter().map(|v| v.to_state()).collect()
        } else {
            vec![]
        }
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

    /// Get finalized tip
    fn get_final_tip(&self) -> Option<HashVal> {
        // for each LNC tip, try to find the final tip
        for tip in self.get_lnc_tips() {
            if let Some(tip) = self.find_final(tip) {
                return Some(tip);
            }
        }
        None
    }

    fn find_final(&self, tip: HashVal) -> Option<HashVal> {
        let mut cursor = self.inner.get_cursor(tip)?;
        loop {
            if cursor.get_streamlet().is_some()
                && cursor.parent()?.get_streamlet().is_some()
                && cursor.parent()?.parent()?.get_streamlet().is_some()
            {
                return Some(cursor.parent()?.header().hash());
            }
            cursor = cursor.parent()?;
        }
    }

    fn get_nonempty_descendants(
        &self,
        mut stack: Vec<Cursor<'_, InMemoryBackend>>,
    ) -> BTreeSet<HashVal> {
        let mut toret = BTreeSet::new();
        while let Some(top) = stack.pop() {
            for child in top.children() {
                if child.get_streamlet().is_some() {
                    toret.insert(child.header().hash());
                }
                stack.push(child);
            }
        }

        toret
    }
}

#[cfg(test)]
mod tests {
    use blkstructs::{GenesisConfig, State};

    use super::*;

    #[test]
    fn simple_sequence() {
        let forest = autosmt::Forest::load(autosmt::MemDB::default());
        let genesis = State::genesis(&forest, GenesisConfig::std_testnet()).seal(None);
        let cstate = ChainState::new(genesis, forest);
        dbg!(cstate.get_lnc_tips());
    }
}
