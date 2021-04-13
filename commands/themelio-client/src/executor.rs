use crate::wallet::manager::WalletManager;
use crate::shell::runner::ShellRunner;
use crate::shell::io::ShellOutput;
use crate::shell::sub::io::SubShellOutput;

/// Responsible for executing a single client CLI command non-interactively.
pub struct CommandExecutor {
    pub host: smol::net::SocketAddr,
    pub database: std::path::PathBuf,
    pub version: String
}

impl CommandExecutor {
    pub fn new(host: smol::net::SocketAddr, database: std::path::PathBuf, version: &str) -> Self {
        let version = version.to_string();
        Self {
            host,
            database,
            version,
        }
    }

    /// Creates a new wallet, stores it into db and outputs the name & secret.
    pub async fn create_wallet(&self, wallet_name: &str) -> anyhow::Result<()> {
        let manager = WalletManager::new(&self.host.clone(), &self.database.clone());
        let (secret, wallet_data) = manager.create_wallet(wallet_name).await?;
        ShellOutput::show_new_wallet(wallet_name, secret, wallet_data).await?;
        Ok(())
    }

    /// Opens a wallet by name and secret and creates a faucet tx to fund the wallet.
    /// The results of the faucet tx from pending to confirm are shown to the user.
    pub async fn faucet(&self, wallet_name: &str, secret: &str, amount: &str, unit: &str) -> anyhow::Result<()> {
        let manager = WalletManager::new(&self.host.clone(), &self.database.clone());
        let wallet_data = manager.load_wallet(wallet_name, secret).await?;

        // new wallet unlock

        // create faucet tx

        // get client snapshot

        // send send using raw

        // query output state


        let res = manager.faucet_transaction(wallet_name, amount, unit).await?;
        SubShellOutput::show_faucet_tx().await?;
        Ok(())
    }

    /// Opens a wallet by name and secret and sends coins from the wallet to a destination.
    pub async fn send_coins(&self, wallet_name: &str, secret: &str, address: &str, amount: &str, unit: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// Adds coins by coin id to wallet.
    pub async fn add_coins(&self, wallet_name: &str, secret: &str, coin_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// Shows the total known wallet balance.
    pub async fn show_balance(&self, wallet_name: &str, secret: &str, ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Shows all the wallets by name that are stored in the db.
    pub async fn show_wallets(&self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Launch shell mode until user exits.
    pub async fn shell(&self) -> anyhow::Result<()> {
        let executor = ShellRunner::new(&self.host, &self.database, &self.version);
        executor.run().await?;
        Ok(())
    }
}