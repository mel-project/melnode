mod client;
mod server;

pub use client::*;
pub use server::*;

use serde::{Deserialize, Serialize};
use themelio_stf::{ConsensusProof, Header, NetID, ProposerAction, Transaction, TxHash};
use tmelcrypt::HashVal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbbreviatedBlock {
    pub header: Header,
    pub proposer_action: Option<ProposerAction>,
    pub txhashes: Vec<TxHash>,
}

impl AbbreviatedBlock {
    pub fn from_state(state: &themelio_stf::SealedState) -> Self {
        let header = state.header();
        let txhashes: Vec<TxHash> = state
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
