use crate::options::{ClientOpts, ClientSubOpts};
use crate::shell::executor::ShellExecutor;
use crate::executor::ClientExecutor;

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
        let host = opts.host.clone();
        let database = opts.database.clone();
        let executor = ClientExecutor::new(host, database);
        match opts.subcommand {
            ClientSubOpts::CreateWallet { wallet_name } => {
                executor.create_wallet(&wallet_name).await?
            }
            ClientSubOpts::Faucet { amount, unit } => {
                executor.faucet(&amount, &unit).await?
            }
            ClientSubOpts::SendCoins { address, amount, unit } => {
                executor.send_coins(&address, &amount, &unit).await?
            }
            ClientSubOpts::AddCoins { coin_id } => {
                executor.add_coins(&coin_id).await?
            }
            ClientSubOpts::ShowBalance => {
                executor.show_balance().await?
            }
            ClientSubOpts::ShowWallets => {
                executor.show_wallets().await?
            }
            ClientSubOpts::Shell => {
                let shell_dispatcher = ShellExecutor::new(&host, &database, &self.version);
                shell_dispatcher.run().await?
            }
        }
        Ok(())
    }
}