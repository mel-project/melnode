use std::path::PathBuf;
use std::{collections::BTreeMap, path::Path};

use anyhow;
use sled;

use storage::SledMap;

use crate::wallet::data::WalletData;

const WALLET_NAMESPACE: &[u8; 14] = b"wallet_storage";

/// Uses sled map(s) to persist client-side data.
pub struct WalletStorage {
    database: SledMap<String, WalletData>,
}

impl WalletStorage {
    /// Creates a new WalletStorage from a path that contains a sled database.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            database: SledMap::<String, WalletData>::new(
                sled::open(&path)?.open_tree(WALLET_NAMESPACE)?,
            ),
        })
    }

    /// Insert wallet_shell data by wallet_shell name.
    pub async fn insert(&self, name: &str, data: &WalletData) -> anyhow::Result<()> {
        self.database.insert(name.to_string(), data.clone());
        Ok(())
    }

    /// Get wallet_shell data given wallet_shell name if it exists.
    pub async fn get(&self, name: &str) -> anyhow::Result<Option<WalletData>> {
        Ok(self.database.get(name))
    }

    /// Get a map of wallet_shell data by name which contains all persisted wallet_shell data.
    pub async fn get_all(&self) -> anyhow::Result<BTreeMap<String, WalletData>> {
        Ok(self.database.get_all().collect())
    }
}
