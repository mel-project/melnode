use crate::wallet::wallet::Wallet;

pub(crate) struct ClientExecutor {
    host: smol::net::SocketAddr,
    database: std::path::PathBuf
}

impl ClientExecutor {
    pub fn new(host: smol::net::SocketAddr, database: std::path::PathBuf) -> Self {
        Self {
            host,
            database
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
        wallet.fuacet(wallet_name).await?;
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
    pub async fn show_wallets(&self, wallet_name: &str) -> anyhow::Result<()> {
        let wallet = self.load_wallet()?;
        Ok(())
    }
    pub async fn exit(&self, wallet_name: &str) -> anyhow::Result<()> {
        let wallet = self.load_wallet()?;
        Ok(())
    }
}