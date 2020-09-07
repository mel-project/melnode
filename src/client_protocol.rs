use blkstructs::{CoinDataHeight, CoinID, Transaction};
use rlp_derive::{RlpDecodable, RlpEncodable};
use tmelcrypt::HashVal;

#[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
pub(crate) struct GetCoinRequest {
    pub coin_id: CoinID,
    pub height: u64,
}

#[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
pub(crate) struct GetCoinResponse {
    pub coin_data: Option<CoinDataHeight>,
    pub compressed_proof: Vec<u8>,
}

#[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
pub(crate) struct GetTxRequest {
    pub txhash: HashVal,
    pub height: u64,
}

#[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
pub(crate) struct GetTxResponse {
    pub transaction: Option<Transaction>,
    pub compressed_proof: Vec<u8>,
}
