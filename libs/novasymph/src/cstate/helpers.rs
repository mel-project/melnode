use std::collections::BTreeMap;

use blkdb::{backends::InMemoryDb, Cursor};
use novasmt::ContentAddrStore;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use themelio_stf::StakeMapping;
use themelio_structs::AbbrBlock;
use thiserror::Error;
use tmelcrypt::{Ed25519PK, HashVal};

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
pub trait CursorExt<C: ContentAddrStore> {
    fn get_streamlet(&self) -> Option<StreamletMetadata>;
    fn chain_weight(&self) -> u64;
}

impl<'a, C: ContentAddrStore> CursorExt<C> for Cursor<'a, InMemoryDb, C> {
    fn get_streamlet(&self) -> Option<StreamletMetadata> {
        let metadata = self.metadata();
        if metadata.is_empty() {
            None
        } else {
            Some(stdcode::deserialize(metadata).unwrap())
        }
    }

    fn chain_weight(&self) -> u64 {
        static MEMOIZER: Lazy<RwLock<BTreeMap<HashVal, u64>>> = Lazy::new(Default::default);

        if let Some(val) = MEMOIZER.read().get(&self.header().hash()) {
            return *val;
        }
        let value = {
            match self.parent() {
                Some(parent) => {
                    let parent_weight =
                        stacker::maybe_grow(32 * 1024, 1024 * 1024, || parent.chain_weight());
                    if self.get_streamlet().is_none() {
                        parent_weight
                    } else {
                        parent_weight + 1
                    }
                }
                None => 1,
            }
        };
        MEMOIZER.write().insert(self.header().hash(), value);
        value
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
    pub fn is_notarized(&self, epoch: u64, stakes: &StakeMapping<impl ContentAddrStore>) -> bool {
        // count the votes
        let mut voting_stake = 0u128;
        let mut total_stake = 0u128;
        for stake in stakes.val_iter() {
            if epoch >= stake.e_start && epoch < stake.e_post_end {
                total_stake += stake.syms_staked.0;
                if self.votes.get(&stake.pubkey).is_some() {
                    voting_stake += stake.syms_staked.0;
                }
            }
        }
        assert!(total_stake >= voting_stake);
        // is this enough?
        voting_stake > 2 * total_stake / 3
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
