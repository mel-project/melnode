use blkstructs::melvm::Covenant;
use crate::wallet::storage::WalletStorage;
use crate::error::ClientError;
use tmelcrypt::Ed25519SK;
use std::collections::BTreeMap;
use std::convert::TryInto;
use crate::wallet::data::WalletData;
use crate::wallet::wallet::Wallet;

/// Responsible for managing storage and network related wallet operations.
pub struct WalletManager {
    host: smol::net::SocketAddr,
    database: std::path::PathBuf,
}

impl WalletManager {
    pub fn new(host: &smol::net::SocketAddr, database: &std::path::PathBuf) -> Self {
        let host = host.clone();
        let database = database.clone();
        Self { host, database }
    }

    /// Create a wallet from wallet name iff name is valid and wallet doesn't already exist.
    pub async fn create_wallet(&self, name: &str) -> anyhow::Result<Wallet> {
        // Check if wallet has only alphanumerics.
        if name.chars().all(char::is_alphanumeric) == false {
            anyhow::bail!(ClientError::InvalidWalletName(name.to_string()))
        }

        let storage = WalletStorage::new(&self.database);
        // Check if wallet with same name already exits.
        if let Some(_stored_wallet_data) = storage.get(name).await? {
            anyhow::bail!(ClientError::DuplicateWalletName(name.to_string()))
        }

        // Generate wallet data and store it.
        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let script = Covenant::std_ed25519_pk(pk);
        let wallet_data = WalletData::new(script.clone());

        // Insert wallet data and return sk & wallet data.
        storage.insert(name, &wallet_data).await?;

        // Return created wallet
        let wallet = Wallet::new(sk, name, wallet_data);
        Ok(wallet)
    }

    /// Get existing wallet data by name given the corresponding secret.
    pub async fn load_wallet(&self, name: &str, secret: &str) -> anyhow::Result<Wallet> {
        let storage = WalletStorage::new(&self.database);
        let wallet_data = storage.get(name).await?.unwrap();

        let secret = secret.clone();
        let wallet_secret = hex::decode(secret)?;
        let sk: Ed25519SK = Ed25519SK(wallet_secret.as_slice().try_into()?);

        if wallet_data.my_script.0 != sk.0 {
            anyhow::bail!(ClientError::InvalidWalletSecret(name.to_string()))
        }

        // Return created wallet
        let wallet = Wallet::new(sk, name, wallet_data);
        Ok(wallet)
    }

    /// Get all wallet data in storage by name.
    pub async fn get_all_wallets(&self) -> anyhow::Result<BTreeMap<String, WalletData>> {
        let storage = WalletStorage::new(&self.database);
        let wallet_data_by_name: BTreeMap<String, WalletData> = storage.get_all().await?;
        Ok(wallet_data_by_name)
    }
}