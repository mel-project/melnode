use std::collections::BTreeMap;
use std::str::FromStr;

use blkstructs::melvm::Covenant;
use tmelcrypt::Ed25519SK;

use crate::utils::context::ExecutionContext;
use crate::wallet::data::WalletData;
use crate::wallet::error::WalletError;
use crate::wallet::wallet::ActiveWallet;

/// Responsible for managing persisted wallets and providing an unlocked active wallet.
pub struct WalletManager {
    context: ExecutionContext,
}

impl WalletManager {
    pub fn new(context: ExecutionContext) -> Self {
        Self { context }
    }

    /// Create a wallet from wallet name iff name is valid and wallet doesn't already exist.
    pub async fn create_wallet(&self, name: &str) -> anyhow::Result<ActiveWallet> {
        // Check if wallet has only alphanumerics.
        if !name.chars().all(char::is_alphanumeric) {
            anyhow::bail!(WalletError::InvalidName(name.to_string()))
        }

        // Check if wallet with same name already exits.
        if let Some(_stored_wallet_data) = self.context.database.get(name) {
            anyhow::bail!(WalletError::DuplicateName(name.to_string()))
        }

        // Generate wallet data and store it.
        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let script = Covenant::std_ed25519_pk(pk);
        let wallet_data = WalletData::new(script);

        // Insert wallet data and return sk & wallet data.
        self.context
            .database
            .insert(name.to_string(), wallet_data.clone());

        // Return created wallet
        let wallet = ActiveWallet::new(sk, name, wallet_data, self.context.clone());
        Ok(wallet)
    }

    /// Get existing wallet data by name given the corresponding secret.
    pub async fn load_wallet(&self, name: &str, secret: &str) -> anyhow::Result<ActiveWallet> {
        let wallet_data = self.context.database.get(name);
        if wallet_data.is_none() {
            anyhow::bail!(WalletError::NotFound(name.to_string()))
        }
        let wallet_data = wallet_data.unwrap();
        let sk = Ed25519SK::from_str(secret);
        if sk.is_err() {
            anyhow::bail!(WalletError::InvalidSecret(name.to_string()))
        }
        let sk = sk?;

        // TODO: add wallet data pk verification
        // let wallet_secret = hex::decode(wallet_secret)?;
        // let wallet_secret = tmelcrypt::Ed25519SK(wallet_secret.as_slice().try_into()?);
        // if melvm::Covenant::std_ed25519_pk(wallet_secret.to_public()) != wallet_shell.my_script {
        //      return Err(anyhow::anyhow!("unlocking failed, make sure you have the right secret!"));
        // }

        let wallet = ActiveWallet::new(sk, name, wallet_data, self.context.clone());
        Ok(wallet)
    }

    /// Get all wallet data in storage by name.
    pub async fn get_all_wallets(&self) -> anyhow::Result<BTreeMap<String, WalletData>> {
        Ok(self.context.database.get_all().collect())
    }
}
