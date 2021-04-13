use crate::wallet::manager::WalletManager;


/// Responsible for executing a single client CLI command.
pub struct CommandExecutor {
    pub host: smol::net::SocketAddr,
    pub database: std::path::PathBuf,
    interactive: bool,

}

impl CommandExecutor {
    pub fn new(host: smol::net::SocketAddr, database: std::path::PathBuf, interactive: bool) -> Self {
        Self {
            host,
            database,
            interactive,
        }
    }

    /// Creates a new wallet, stores it into db and outputs the name & secret.
    pub async fn create_wallet(&self, wallet_name: &str) -> anyhow::Result<()> {
        let wallet = WalletManager::new(&self.host.clone(), &self.database.clone());
        let wallet_data = wallet.
        ClientOutput::show_new_wallet(wallet_name, wallet_data);
        ClientPrompter::show_new_wallet
        let wallet = self.load_wallet()?;
        wallet.create(wallet_name);

        ClientOutput::show_new_wallet(wallet);
        Ok(())
    }

    /// Opens a wallet by name and secret and creates a faucet tx to wallet.
    /// The results of the faucet tx are shown to the user.
    pub async fn faucet(&self, wallet_name: &str, secret: &str, amount: &str, unit: &str) -> anyhow::Result<()> {
        let wallet = self.load_wallet()?;
        let wallet.
        wallet.faucet(&wallet, amount, unit).await?;
        Ok(())
    }

    /// Opens a wallet by name and secret and sends coins from the wallet to a destination.
    pub async fn send_coins(&self, wallet_name: &str, secret: &str, address: &str, amount: &str, unit: &str) -> anyhow::Result<()> {
        wallet.send_coins(wallet_name).await?;
        Ok(())
    }

    /// Adds coins by coin id to wallet.
    pub async fn add_coins(&self, wallet_name: &str, secret: &str, coin_id: &str) -> anyhow::Result<()> {
        wallet.add_coins(wallet_name).await?;
        Ok(())
    }

    /// Shows the total known wallet balance.
    pub async fn show_balance(&self, wallet_name: &str, secret: &str, ) -> anyhow::Result<()> {
        wallet.show_balance().await?;
        Ok(())
    }

    /// Shows all the wallets by name that are stored in the db.
    pub async fn show_wallets(&self) -> anyhow::Result<()> {
        Ok(())
    }

}