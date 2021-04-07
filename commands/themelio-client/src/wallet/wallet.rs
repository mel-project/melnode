use crate::wallet::data::WalletData;
use blkstructs::melvm::Covenant;
use crate::storage::WalletStorage;
use crate::wallet::error::ClientError;
use tmelcrypt::Ed25519SK;
use std::collections::BTreeMap;
use std::convert::TryInto;

pub struct Wallet {
    host: smol::net::SocketAddr,
    database: std::path::PathBuf
}

impl Wallet {
    pub fn new(host: &smol::net::SocketAddr, database: &std::path::PathBuf) -> Self {
        let host = host.clone();
        let database = database.clone();
        Self { host, database }
    }

    /// Create a wallet from wallet name if name is valid and wallet doesn't already exist
    pub async fn create(&self, name: &str) -> anyhow::Result<(Ed25519SK, WalletData)> {
        // Check if wallet has only alphanumerics
        if name.chars().all(char::is_alphanumeric) == false {
            anyhow::bail!(ClientError::InvalidWalletName(name.to_string()))
        }

        let storage = WalletStorage::new(&self.database);
        // Check if wallet with same name already exits
        if let Some(_stored_wallet_data) = storage.get(name).await? {
            anyhow::bail!(ClientError::WalletDuplicate(name.to_string()))
        }

        // Generate wallet data and store it
        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let script = Covenant::std_ed25519_pk(pk);
        let wallet_data = WalletData::new(script.clone());

        // Insert wallet data and return sk & wallet data
        storage.insert(name, &wallet_data).await?;
        Ok((sk, wallet_data))
    }

    /// Get all wallet data in storage by name
    pub async fn get_all(&self) -> anyhow::Result<BTreeMap<String, WalletData>> {
        let storage = WalletStorage::new(&self.database);
        Ok(storage.get_all().await?)
    }

    /// Get existing wallet data by name
    pub async fn open(&self, name: &str, secret: &str) -> anyhow::Result<WalletData> {
        let storage = WalletStorage::new(&self.database);
        let wallet_data = storage.get(name).await?.unwrap();

        let secret = secret.clone();
        let wallet_secret = hex::decode(secret)?;
        let sk: Ed25519SK = Ed25519SK(wallet_secret.as_slice().try_into()?);

        if wallet_data.my_script.0 != sk.0 {
            anyhow::bail!(ClientError::InvalidWalletSecret(name.to_string()))
        }

        Ok(wallet_data)
    }
}