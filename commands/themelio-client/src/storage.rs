use anyhow;
use sled;
use storage::SledMap;
use std::path::PathBuf;
use crate::wallet::data::WalletData;

const WALLET_NAMESPACE: &[u8; 6] = b"wallet";

/// Uses sled map(s) to persist client-side data
pub struct ClientStorage {
    path: PathBuf,
}

impl ClientStorage {
    pub fn new(path: &PathBuf) -> Self {
        Self {
            path: path.clone(),
        }
    }

    /// Insert wallet data by wallet name
    pub async fn insert_wallet(&self, name: &String, data: &WalletData) -> anyhow::Result<()> {
        let db = sled::open(&self.path)?;
        let tree = db.open_tree(WALLET_NAMESPACE)?;
        let map = SledMap::<String, WalletData>::new(tree);
        map.insert(name.clone(), data.clone());
        Ok(())
    }

    /// Get wallet data given wallet name if it exists
    pub async fn get_wallet_by_name(&self, name: &String) -> anyhow::Result<Option<WalletData>> {
        let db = sled::open(&self.path)?;
        let tree = db.open_tree(WALLET_NAMESPACE)?;
        let map = SledMap::<String, WalletData>::new(tree);
        let wallet_data = map.get(name);
        Ok(wallet_data)
    }
}
