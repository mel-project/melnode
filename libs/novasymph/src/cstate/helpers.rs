use std::collections::BTreeMap;

use blkdb::{backends::InMemoryBackend, Cursor};
use blkstructs::{AbbrBlock, StakeMapping};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tmelcrypt::Ed25519PK;

use crate::msg::{ProposalSig, VoteSig};

#[derive(Error, Debug)]
pub enum ProposalError {
    #[error("proposal is an invalid block")]
    InvalidBlock,
    #[error("proposal not at the right height")]
    IncorrectHeight,
    #[error("proposal doesn't extend the longest notarized chain")]
    NotExtendingLnc,
}

#[derive(Error, Debug)]
pub enum VoteError {
    #[error("voting for nonexistent block")]
    NoSuchBlock,
    #[error("voting for empty block")]
    EmptyBlock,
    #[error("invalid signature")]
    InvalidSignature,
}

/// An extension trait for dealing with Cursors.
pub trait CursorExt {
    fn get_streamlet(&self) -> Option<StreamletMetadata>;
    fn chain_weight(&self) -> u64;
}

impl<'a> CursorExt for Cursor<'a, InMemoryBackend> {
    fn get_streamlet(&self) -> Option<StreamletMetadata> {
        let metadata = self.metadata();
        if metadata.is_empty() {
            None
        } else {
            Some(stdcode::deserialize(metadata).unwrap())
        }
    }

    fn chain_weight(&self) -> u64 {
        let mut tip = self.clone();
        let mut weight = if tip.get_streamlet().is_some() { 1 } else { 0 };
        while let Some(parent) = tip.parent() {
            weight += if parent.get_streamlet().is_some() {
                1
            } else {
                0
            };
            tip = parent;
        }
        weight
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StreamletMetadata {
    pub proposer: Ed25519PK,
    pub proposal_sig: ProposalSig,
    pub votes: BTreeMap<Ed25519PK, VoteSig>,
}

impl StreamletMetadata {
    /// Returns whether or not this block is notarized, given the stake mapping and epoch.
    pub fn is_notarized(&self, epoch: u64, stakes: &StakeMapping) -> bool {
        // count the votes
        let mut voting_stake = 0u128;
        let mut total_stake = 0u128;
        for stake in stakes.val_iter() {
            if epoch >= stake.e_start && epoch < stake.e_post_end {
                total_stake += stake.syms_staked;
                if self.votes.get(&stake.pubkey).is_some() {
                    voting_stake += stake.syms_staked;
                }
            }
        }
        assert!(total_stake >= voting_stake);
        // is this enough?
        voting_stake >= 2 * total_stake / 3
    }

    /// Checks that the proposal and votes actually belong to the given block.
    pub fn is_signed_correctly(&self, voting_for: &AbbrBlock) -> bool {
        if !self.proposal_sig.verify(self.proposer, voting_for) {
            return false;
        }
        for (voter, vote) in self.votes.iter() {
            if !vote.verify(*voter, voting_for.header.hash()) {
                return false;
            }
        }
        true
    }
}
