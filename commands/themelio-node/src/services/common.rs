pub use anyhow::Result;
pub use parking_lot::RwLock;
pub use smol::prelude::*;

use serde::{Deserialize, Serialize};
pub use smol::{Task, Timer};

use themelio_stf::Transaction;
use tmelcrypt::HashVal;
/// Request for a new block.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewBlkRequest {
    pub header: themelio_stf::Header,
    pub txhashes: Vec<HashVal>,
    pub partial_transactions: Vec<Transaction>,
}

/// Response for a new block request.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewBlkResponse {
    pub missing_txhashes: Vec<HashVal>,
}
