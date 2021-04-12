use crate::wallet::wallet::Wallet;

pub(crate) struct WalletAdapter {
    host: smol::net::SocketAddr,
    database: std::path::PathBuf,
    interactive: bool,
}

impl WalletAdapter {
    pub fn new(host: smol::net::SocketAddr, database: std::path::PathBuf, interactive: bool) -> Self {
        Self {
            host,
            database,
            interactive,
        }
    }

    fn load_wallet(&self) -> anyhow::Result<Wallet> {
        let wallet = Wallet::new(&self.host.clone(), &self.database.clone());
        Ok(wallet)
    }
    pub async fn create_wallet(&self, wallet_name: &str) -> anyhow::Result<()> {
        let wallet = self.load_wallet()?;
        wallet.create(wallet_name);
        Ok(())
    }
    pub async fn faucet(&self, amount: &str, unit: &str) -> anyhow::Result<()> {
        let wallet = self.load_wallet()?;
        wallet.faucet(amount, unit).await?;
        Ok(())
    }
    pub async fn send_coins(&self, address: &str, amount: &str, unit: &str) -> anyhow::Result<()> {
        let wallet = self.load_wallet()?;
        wallet.send_coins(wallet_name).await?;
        Ok(())
    }
    pub async fn add_coins(&self, coin_id: &str) -> anyhow::Result<()> {
        let wallet = self.load_wallet()?;
        wallet.add_coins(wallet_name).await?;
        Ok(())
    }
    pub async fn show_balance(&self) -> anyhow::Result<()> {
        let wallet = self.load_wallet()?;
        wallet.show_balance().await?;
        Ok(())
    }
    pub async fn show_wallets(&self) -> anyhow::Result<()> {
        let wallet = self.load_wallet()?;
        Ok(())
    }

}