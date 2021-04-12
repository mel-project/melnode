use crate::options::{ClientOpts, ClientSubOpts};
use crate::shell::dispatcher::ShellDispatcher;

pub(crate) struct ClientDispatcher {
    opts: ClientOpts,
    version: String
}

impl ClientDispatcher {
    pub fn new(opts: ClientOpts, version: &str) -> Self {
        let version = version.to_string();
        Self { opts, version }
    }

    pub async fn dispatch(&self) -> anyhow::Result<()> {
        match self.opts.subcommand {
            ClientSubOpts::CreateWallet { .. } => {}
            ClientSubOpts::Faucet { .. } => {}
            ClientSubOpts::SendCoins { .. } => {}
            ClientSubOpts::AddCoins { .. } => {}
            ClientSubOpts::ShowBalance => {}
            ClientSubOpts::ShowWallets => {}
            ClientSubOpts::Shell => {
                let shell_dispatcher = ShellDispatcher::new(&self.opts.host, &self.opts.database, &self.version);
                shell_dispatcher.run().await?
            }
            ClientSubOpts::Exit => {}
        }
        Ok(())
    }
}