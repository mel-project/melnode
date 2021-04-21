mod client;
mod server;

pub use client::*;
pub use server::*;

use blkstructs::{ConsensusProof, Header, NetID, ProposerAction, Transaction};
use serde::{Deserialize, Serialize};
use tmelcrypt::HashVal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbbreviatedBlock {
    pub header: Header,
    pub proposer_action: Option<ProposerAction>,
    pub txhashes: Vec<HashVal>,
}

impl AbbreviatedBlock {
    pub fn from_state(state: &blkstructs::SealedState) -> Self {
        let header = state.header();
        let txhashes: Vec<HashVal> = state
            .inner_ref()
            .transactions
            .val_iter()
            .map(|v| v.hash_nosigs())
            .collect();
        Self {
            header,
            txhashes,
            proposer_action: state.proposer_action().cloned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSummary {
    pub netid: NetID,
    pub height: u64,
    pub header: Header,
    pub proof: ConsensusProof,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub enum Substate {
    History,
    Coins,
    Transactions,
    Pools,
    Stakes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeRequest {
    SendTx(Transaction),
    GetAbbrBlock(u64),
    GetSummary,
    GetSmtBranch(u64, Substate, HashVal),
    GetStakersRaw(u64),
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
