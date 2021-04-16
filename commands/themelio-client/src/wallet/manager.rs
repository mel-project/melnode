use std::collections::BTreeMap;
use std::str::FromStr;

use blkstructs::melvm::Covenant;
use tmelcrypt::Ed25519SK;

use crate::common::context::ExecutionContext;
use crate::wallet::data::WalletData;
use crate::wallet::error::ClientError;
use crate::wallet::storage::WalletStorage;
use crate::wallet::wallet::Wallet;

/// Responsible for managing storage and network related wallet operations.
pub struct WalletManager {
    context: ExecutionContext,
}

impl WalletManager {
    pub fn new(context: ExecutionContext) -> Self {
        Self { context }
    }

    /// Create a wallet from wallet name iff name is valid and wallet doesn't already exist.
    pub async fn create_wallet(&self, name: &str) -> anyhow::Result<Wallet> {
        // Check if wallet has only alphanumerics.
        if name.chars().all(char::is_alphanumeric) == false {
            anyhow::bail!(ClientError::InvalidWalletName(name.to_string()))
        }

        let storage = WalletStorage::new(&self.context.database);
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
        let wallet = Wallet::new(sk, name, wallet_data, self.context.clone());
        Ok(wallet)
    }

    /// Get existing wallet data by name given the corresponding secret.
    pub async fn load_wallet(&self, name: &str, secret: &str) -> anyhow::Result<Wallet> {
        let storage = WalletStorage::new(&self.context.database);
        let wallet_data = storage.get(name).await?.unwrap();
        let sk = Ed25519SK::from_str(secret.clone()).unwrap();

        // TODO: add wallet data pk verification
        // let wallet_secret = hex::decode(wallet_secret)?;
        // let wallet_secret = tmelcrypt::Ed25519SK(wallet_secret.as_slice().try_into()?);
        // if melvm::Covenant::std_ed25519_pk(wallet_secret.to_public()) != interactive.my_script {
        //      return Err(anyhow::anyhow!("unlocking failed, make sure you have the right secret!"));
        // }

        let wallet = Wallet::new(sk, name, wallet_data, self.context.clone());
        Ok(wallet)
    }

    /// Get all wallet data in storage by name.
    pub async fn get_all_wallets(&self) -> anyhow::Result<BTreeMap<String, WalletData>> {
        let storage = WalletStorage::new(&self.context.database);
        let wallet_data_by_name: BTreeMap<String, WalletData> = storage.get_all().await?;
        Ok(wallet_data_by_name)
    }
}
