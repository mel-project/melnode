use serde::{Deserialize, Serialize};
use themelio_stf::{CoinDataHeight, CoinID, Transaction};
use tmelcrypt::HashVal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GetCoinRequest {
    pub coin_id: CoinID,
    pub height: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GetCoinResponse {
    pub coin_data: Option<CoinDataHeight>,
    pub compressed_proof: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GetTxRequest {
    pub txhash: HashVal,
    pub height: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GetTxResponse {
    pub transaction: Option<Transaction>,
    pub compressed_proof: Vec<u8>,
}
