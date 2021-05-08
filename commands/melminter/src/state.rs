use std::path::Path;

use blkstructs::{CoinData, CoinDataHeight, CoinID, Transaction, TxKind};
use serde::{Deserialize, Serialize};
use tmelcrypt::HashVal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintState {
    pub chain_tip_id: CoinID,
    pub chain_tip_cdh: CoinDataHeight,
    pub chain_tip_hash: HashVal,
    pub payout_covhash: HashVal,
}

impl MintState {
    pub async fn read_from_file(fname: &Path) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(&smol::fs::read(fname).await?)?)
    }

    pub async fn write_to_file(&self, fname: &Path) -> anyhow::Result<()> {
        // TODO: atomically do this
        smol::fs::write(fname, serde_json::to_vec_pretty(self)?).await?;
        Ok(())
    }

    /// Creates a partially-filled-in transaction, with the given difficulty, that's neither signed nor feed. The caller should fill in the DOSC output.
    pub async fn mint_transaction(&self, difficulty: usize) -> Transaction {
        let chi = tmelcrypt::hash_keyed(
            &self.chain_tip_hash,
            &stdcode::serialize(&self.chain_tip_id).unwrap(),
        );
        log::warn!("chi = {}", chi);
        let proof = smol::unblock(move || melpow::Proof::generate(&chi, difficulty)).await;
        log::warn!("proof = {}", hex::encode(&proof.to_bytes()));
        let difficulty = difficulty as u32;
        Transaction {
            kind: TxKind::DoscMint,
            inputs: vec![self.chain_tip_id],
            data: stdcode::serialize(&(difficulty, proof.to_bytes())).unwrap(),
            outputs: vec![self.chain_tip_cdh.coin_data.clone()],
            fee: 0,
            scripts: vec![],
            sigs: vec![],
        }
    }
}
