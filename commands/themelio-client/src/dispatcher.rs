use crate::options::{ClientOpts, ClientSubOpts};
use crate::shell::executor::ShellExecutor;
use crate::adapter::WalletAdapter;

pub struct ClientDispatcher {
    opts: ClientOpts,
    version: String
}

impl ClientDispatcher {
    pub fn new(opts: ClientOpts, version: &str) -> Self {
        let version = version.to_string();
        Self { opts, version }
    }

    pub async fn dispatch(&self) -> anyhow::Result<()> {
        let opts = self.opts.clone();
        let adapter = WalletAdapter::new(opts.host.clone(), opts.database.clone(), false);
        match opts.subcommand {
            ClientSubOpts::CreateWallet { wallet_name } => {
                adapter.create_wallet(&wallet_name).await?
            }
            ClientSubOpts::Faucet { amount, unit } => {
                adapter.faucet(&amount, &unit).await?
            }
            ClientSubOpts::SendCoins { address, amount, unit } => {
                adapter.send_coins(&address, &amount, &unit).await?
            }
            ClientSubOpts::AddCoins { coin_id } => {
                adapter.add_coins(&coin_id).await?
            }
            ClientSubOpts::ShowBalance => {
                adapter.show_balance().await?
            }
            ClientSubOpts::ShowWallets => {
                adapter.show_wallets().await?
            }
            ClientSubOpts::Shell => {
                let executor = ShellExecutor::new(&opts.host.clone(), &opts.database.clone(), &self.version);
                executor.run().await?
            }
        }
        Ok(())
    }
}