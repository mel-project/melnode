use crate::command::ClientSubcommand;

pub(crate) struct ClientDispatcher {}

impl ClientDispatcher {
    pub fn new() -> Self {
        ClientDispatcher {
            
        }
    }
    pub async fn dispatcher(subcommand: ClientSubcommand) -> anyhow::Result<()> {
        match subcommand {
            ClientSubcommand::CreateWallet { .. } => {}
            ClientSubcommand::Faucet { .. } => {}
            ClientSubcommand::SendCoins { .. } => {}
            ClientSubcommand::AddCoins { .. } => {}
            ClientSubcommand::ShowBalance => {}
            ClientSubcommand::ShowWallets => {}
            ClientSubcommand::Shell => {
                let dispatcher = ShellDispatcher::new(&opts.host, &opts.database, version);
            }
            ClientSubcommand::Exit => {}
        }
        Ok(())
    }
}