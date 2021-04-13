use anyhow;
use sled;
use std::collections::BTreeMap;
use std::path::PathBuf;
use storage::SledMap;
use crate::wallet::data::WalletData;

const WALLET_NAMESPACE: &[u8; 5] = b"shell";

/// Uses sled map(s) to persist client-side data.
pub struct WalletStorage {
    path: PathBuf,
}

impl WalletStorage {
    pub fn new(path: &PathBuf) -> Self {
        Self { path: path.clone() }
    }

    /// Insert shell data by shell name.
    pub async fn insert(&self, name: &str, data: &WalletData) -> anyhow::Result<()> {
        let db = sled::open(&self.path)?;
        let tree = db.open_tree(WALLET_NAMESPACE)?;
        let map = SledMap::<String, WalletData>::new(tree);
        map.insert(name.to_string(), data.clone());
        Ok(())
    }

    /// Get shell data given shell name if it exists.
    pub async fn get(&self, name: &str) -> anyhow::Result<Option<WalletData>> {
        let db = sled::open(&self.path)?;
        let tree = db.open_tree(WALLET_NAMESPACE)?;
        let map = SledMap::<String, WalletData>::new(tree);
        let wallet_data = map.get(&name.to_string());
        Ok(wallet_data)
    }

    /// Get a map of shell data by name which contains all persisted shell data.
    pub async fn get_all(&self) -> anyhow::Result<BTreeMap<String, WalletData>> {
        let db = sled::open(&self.path).unwrap();
        let tree = db.open_tree(WALLET_NAMESPACE).unwrap();
        let map = SledMap::<String, WalletData>::new(tree);
        Ok(map.get_all().collect())
    }

    /// Insert shell data by shell name.
    pub async fn remove(&self, name: &str) -> anyhow::Result<()> {
       todo!("Not implemented")
    }
}
