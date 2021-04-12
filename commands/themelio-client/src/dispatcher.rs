use crate::options::{ClientOpts, ClientSubOpts};
use crate::shell::dispatcher::ShellDispatcher;
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
        let host = self.opts.host.clone();
        let database = self.opts.database.clone();
        let executor = ClientExecutor::new(host, database);
        match self.opts.subcommand {
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
                let shell_dispatcher = ShellDispatcher::new(&host, &database, &self.version);
                shell_dispatcher.run().await?
            }
            ClientSubOpts::Exit => {
                executor.exit().await?
            }
        }
        Ok(())
    }
}