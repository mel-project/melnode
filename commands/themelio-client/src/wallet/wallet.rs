use blkstructs::melvm::Covenant;
use crate::wallet::storage::WalletStorage;
use crate::error::ClientError;
use tmelcrypt::Ed25519SK;
use std::collections::BTreeMap;
use std::convert::TryInto;
use crate::wallet::data::WalletData;

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

    /// Loads existing shell if shell name exists and can be unlocked using secret
    pub async fn load(host: &smol::net::SocketAddr, database: &std::path::PathBuf, name: &str, secret: &str) -> anyhow::Result<Wallet> {
        let wallet = Wallet::new(host, database);
        let _wallet_data = wallet.open(name, secret).await?;
        Ok(wallet)
    }

    /// Create a shell from shell name if name is valid and shell doesn't already exist
    pub async fn create(&self, name: &str) -> anyhow::Result<(Ed25519SK, WalletData)> {
        // Check if shell has only alphanumerics
        if name.chars().all(char::is_alphanumeric) == false {
            anyhow::bail!(ClientError::InvalidWalletName(name.to_string()))
        }

        let storage = WalletStorage::new(&self.database);
        // Check if shell with same name already exits
        if let Some(_stored_wallet_data) = storage.get(name).await? {
            anyhow::bail!(ClientError::WalletDuplicate(name.to_string()))
        }

        // Generate shell data and store it
        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let script = Covenant::std_ed25519_pk(pk);
        let wallet_data = WalletData::new(script.clone());

        // Insert shell data and return sk & shell data
        storage.insert(name, &wallet_data).await?;
        Ok((sk, wallet_data))
    }

    /// Get all shell data in storage by name
    pub async fn get_all(&self) -> anyhow::Result<BTreeMap<String, WalletData>> {
        let storage = WalletStorage::new(&self.database);
        let wallet_data_by_name: BTreeMap<String, WalletData> = storage.get_all().await?;
        Ok(storage.get_all().await?)
    }

    /// Get existing shell data by name
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