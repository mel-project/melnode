use blkstructs::melvm::Covenant;
use crate::wallet::storage::WalletStorage;
use crate::error::ClientError;
use tmelcrypt::{Ed25519SK, HashVal};
use std::collections::BTreeMap;
use std::convert::TryInto;
use crate::wallet::data::WalletData;
use blkstructs::CoinID;

/// Responsible for managing storage and network related wallet operations.
pub struct WalletManager {
    host: smol::net::SocketAddr,
    database: std::path::PathBuf
}

impl WalletManager {
    pub fn new(host: &smol::net::SocketAddr, database: &std::path::PathBuf) -> Self {
        let host = host.clone();
        let database = database.clone();
        Self { host, database }
    }

    /// Create a wallet from wallet name iff name is valid and wallet doesn't already exist.
    pub async fn create_wallet(&self, name: &str) -> anyhow::Result<(Ed25519SK, WalletData)> {
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
        Ok((sk, wallet_data))
    }

    /// Loads existing wallet iff wallet name exists and can be unlocked using secret.
    pub async fn load_wallet(host: &smol::net::SocketAddr, database: &std::path::PathBuf, name: &str, secret: &str) -> anyhow::Result<WalletManager> {
        let wallet = WalletManager::new(host, database);
        let _wallet_data = wallet.open(name, secret).await?;
        Ok(wallet)
    }


    /// Get all wallet data in storage by name.
    pub async fn get_all_wallets(&self) -> anyhow::Result<BTreeMap<String, WalletData>> {
        let storage = WalletStorage::new(&self.database);
        let wallet_data_by_name: BTreeMap<String, WalletData> = storage.get_all().await?;
        Ok(wallet_data_by_name)
    }

    /// Get existing wallet data by name given the corresponding secret.
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

    /// Use faucet to mint mels.
    pub async fn faucet(&self, mut wallet_data: &WalletData, amt: u128, denom: &[u8] ) -> anyhow::Result<CoinID> {

        // let shell = Wallet::new(&self.host, &self.database);
        //
        // let wallet_data = shell.sub(&self.name, &self.secret).await?;
        //
        // let coin = shell.faucet(&wallet_data, self.amt, self.denom).await?;
        //
        // prompter::output_faucet_tx(wallet_data, coin).await?;
        //
        // self.confirm_faucet_tx(coin).await?;
        //
        // prompter::faucet_tx_confirmed().await?;
        Ok(CoinID{ txhash: Default::default(), index: 0 })
    }

    async fn confirm_faucet(&self, _coin_id: CoinID) -> anyhow::Result<()> {
        // loop {
        //
        //     prompter::faucet_tx_confirming().await?;
        // }
        //                 eprintln!(
//                     ">> Faucet transaction for {} mels broadcast!",
//                     number.to_string().bold()
//                 );
//                 eprintln!(">> Waiting for confirmation...");
//                 // loop until we get coin data height and proof from last header
//                 loop {
//                     let (coin_data_height, _hdr) = active_wallet.get_coin_data(coin).await?;
//                     if let Some(cd_height) = coin_data_height {
//                         eprintln!(
//                             ">>> Coin is confirmed at current height {}",
//                             cd_height.height
//                         );

//                         eprintln!(
//                             ">> CID = {}",
//                             hex::encode(stdcode::serialize(&coin).unwrap()).bold()
//                         );
//                         break;
//                     }
        Ok(())
    }

    /// Send coins to a recipient.
    pub async fn send_coins(&self, mut wallet_data: &WalletData, dest: HashVal, amt: u128, denom: &[u8]) -> anyhow::Result<CoinID> {
        Ok(CoinID{ txhash: Default::default(), index: 0 })
    }

    /// Add coins to this wallet
    pub async fn add_coins(&self, wallet_data: &WalletData, ) -> anyhow::Result<CoinID> {
        Ok(CoinID{ txhash: Default::default(), index: 0 })
    }

    /// Check the balance for this wallet.
    pub async fn balance(&self, wallet_data: &WalletData, ) -> anyhow::Result<CoinID> {
        Ok(CoinID{ txhash: Default::default(), index: 0 })
    }
}