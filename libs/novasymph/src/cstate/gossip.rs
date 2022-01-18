use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use themelio_structs::{AbbrBlock, Block, Transaction, TxHash};
use tmelcrypt::HashVal;

use super::helpers::StreamletMetadata;

/// A gossip request that contains the info needed to solicit some newer info from a peer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockRequest {
    /// Contains the LNC tips.
    pub lnc_tips: BTreeSet<HashVal>,
}

/// A gossip response that contains information for *one* block.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AbbrBlockResponse {
    pub abbr_block: AbbrBlock,
    pub metadata: StreamletMetadata,
    pub last_nonempty: HashVal,
}

/// A gossip response that contains information for one block, that has all the information needed
#[derive(Clone, Debug)]
pub struct FullBlockResponse {
    pub block: Block,
    pub metadata: StreamletMetadata,
    pub last_nonempty: HashVal,
}

/// A gossip request that solicits information about transactions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionRequest {
    /// Which block to search in
    pub block_hash: HashVal,
    /// Transaction hashes
    pub hashes: Vec<TxHash>,
}

/// A gossip response that contains information about transactions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionResponse {
    /// Just the transactions. Hash these transactions to check the txhash validity.
    pub transactions: Vec<Transaction>,
}
