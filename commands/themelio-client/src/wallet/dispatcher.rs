use crate::wallet::command::WalletCommand;
use crate::wallet::open::command::OpenWalletCommand;
use crate::wallet::wallet::Wallet;
use crate::wallet::prompt;
use crate::wallet::open::dispatcher as open_dispatcher;

pub struct Dispatcher {
    host: smol::net::SocketAddr,
    database: std::path::PathBuf,
    version: String
}

impl Dispatcher {
    pub(crate) fn new(host: &smol::net::SocketAddr, database: &std::path::PathBuf, version: &str) -> Self {
        let host = host.clone();
        let database = database.clone();
        let version = version.to_string();
        Self { host, database, version }
    }

    /// Dispatch commands from user input and show output using prompt until user exits.
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        let prompt = prompt::format_prompt(&self.version).await?;

        loop {
            // Get command from user input
            let (cmd, open_cmd) = prompt::input_command(&prompt).await?;

            // Exit if the user chooses to exit
            if cmd == WalletCommand::Exit {
                return Ok(());
            }

            // Dispatch the command
            let dispatch_result = &self.dispatch(&cmd, &open_cmd).await;

            // Output error, if any, and continue running
            match dispatch_result {
                Err(err) => prompt::output_cmd_error(err, &cmd).await?,
                _ => {}
            }
        }
    }

    /// Parse user input into a wallet command process the command
    async fn dispatch(&self, cmd: &WalletCommand, open_cmd: &Option<OpenWalletCommand>) -> anyhow::Result<()> {
        // Dispatch a command and return a command result
        match &cmd {
            WalletCommand::Create(name) => self.create(name).await,
            WalletCommand::Show => self.show().await,
            WalletCommand::Open(name, secret) => self.open(name, secret).await,
            WalletCommand::Use(name, secret) => self.use_(name, secret, open_cmd).await,
            WalletCommand::Delete(name) => self.delete(name).await,
            WalletCommand::Help => self.help().await,
            WalletCommand::Exit => { self.exit().await }
        }
    }

    /// Create wallet given a valid and unique name and store it
    async fn create(&self, name: &str) -> anyhow::Result<()> {
        let wallet = Wallet::new(&self.host, &self.database);
        let (sk, wallet_data) = wallet.create(name).await?;
        prompt::new_wallet_info(name, sk, &wallet_data);
        Ok(())
    }

    async fn delete(&self, name: &str) -> anyhow::Result<()> {
        todo!("Not implemented")
    }

    /// Shows all stored wallet names and the corresponding wallet address
    async fn show(&self) -> anyhow::Result<()> {
        let wallet = Wallet::new(&self.host, &self.database);
        let wallets = wallet.get_all().await?;
        prompt::wallets_info(wallets).await;
        Ok(())
    }

    /// If wallet does not exist finish the open command,
    /// otherwise run commands in open wallet mode until exit command.
    async fn open(
        &self,
        name: &str,
        secret: &str,
    ) -> anyhow::Result<()> {
        let dispatcher = open_dispatcher::Dispatcher::new(&opts.host, &opts.database, version, name);

        dispatcher.run().await?;

        Ok(());

    }

    /// Use a particular wallet to run a single open wallet command
    async fn use_(
        &self,
        name: &str,
        secret: &str,
        open_wallet_command: &Option<OpenWalletCommand>,
    ) -> anyhow::Result<()> {
        let dispatcher = open_dispatcher::Dispatcher::new(&self.host, &self.database, &self.version, name);
        dispatcher.run_once().await?;
        Ok(());
    }

    async fn help(&self) -> anyhow::Result<()> {
        prompt::output_help().await?;
        Ok(())
    }

    async fn exit(&self) -> anyhow::Result<()> {
        prompt::output_exit().await?;
        Ok(())
    }
}
