use crate::wallet::command::WalletCommand;
use crate::wallet::open::command::OpenWalletCommand;
use crate::wallet::wallet::Wallet;
use crate::wallet::prompter::{Input, Output};
use crate::wallet::open::dispatcher::OpenWalletDispatcher;

pub struct WalletDispatcher {
    host: smol::net::SocketAddr,
    database: std::path::PathBuf,
    version: String
}

impl WalletDispatcher {
    pub(crate) fn new(host: &smol::net::SocketAddr, database: &std::path::PathBuf, version: &str) -> Self {
        let host = host.clone();
        let database = database.clone();
        let version = version.to_string();
        Self { host, database, version }
    }

    /// Dispatch commands from user input and show output using prompt until user exits.
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        // Format user prompt.
        let prompt = Input::format_prompt(&self.version).await?;

        loop {
            // Get command from user input.
            let (cmd, open_cmd) = Input::command(&prompt).await?;

            // Exit if the user chooses to exit.
            if cmd == WalletCommand::Exit {
                Output::exit().await?;
                return Ok(());
            }

            // Dispatch the command (with an optional 'open' command for 'use' single-line mode).
            let dispatch_result = &self.dispatch(&cmd, &open_cmd).await;

            // Output error, if any, and continue running.
            match dispatch_result {
                Err(err) => Output::error(err, &cmd).await?,
                _ => {}
            }
        }
    }

    /// Dispatch and process the command.
    async fn dispatch(&self, cmd: &WalletCommand, open_cmd: &Option<OpenWalletCommand>) -> anyhow::Result<()> {
        // Dispatch a command and return a command result.
        match &cmd {
            WalletCommand::Create(name) => self.create(name).await,
            WalletCommand::Show => self.show().await,
            WalletCommand::Open(name, secret) => self.open(name, secret).await,
            WalletCommand::Use(name, secret) => self.use_(name, secret, &open_cmd).await,
            WalletCommand::Delete(name) => self.delete(name).await,
            WalletCommand::Help => self.help().await,
            WalletCommand::Exit => { self.exit().await }
        }
    }

    /// Create a new wallet and output it's information to user.
    async fn create(&self, name: &str) -> anyhow::Result<()> {
        let wallet = Wallet::new(&self.host, &self.database);
        let (sk, wallet_data) = wallet.create(name).await?;
        Output::wallet(name, sk, &wallet_data);
        Ok(())
    }

    /// Delete an existing wallet.
    async fn delete(&self, _name: &str) -> anyhow::Result<()> {
        todo!("Not implemented")
    }

    /// Shows all stored wallet data.
    async fn show(&self) -> anyhow::Result<()> {
        let wallet = Wallet::new(&self.host, &self.database);
        let wallets = wallet.get_all().await?;
        Output::wallets(wallets).await;
        Ok(())
    }

    /// Open a wallet given the name and secret and run in open wallet dispatch mode.
    async fn open(
        &self,
        name: &str,
        secret: &str,
    ) -> anyhow::Result<()> {
        let dispatcher = OpenWalletDispatcher::new(&self.host, &self.database, &self.version, name, secret).await?;
        dispatcher.run().await?;
        Ok(())
    }

    /// Use a particular wallet to dispatch a single open wallet command.
    async fn use_(
        &self,
        name: &str,
        secret: &str,
        open_wallet_command: &Option<OpenWalletCommand>,
    ) -> anyhow::Result<()> {
        let dispatcher = OpenWalletDispatcher::new(&self.host, &self.database, &self.version, name, secret).await?;
        dispatcher.dispatch(&open_wallet_command.clone().unwrap()).await?;
        Ok(())
    }

    /// Output help message to user.
    async fn help(&self) -> anyhow::Result<()> {
        Output::help().await?;
        Ok(())
    }

    /// Output exit message to user.
    async fn exit(&self) -> anyhow::Result<()> {
        Output::exit().await?;
        Ok(())
    }
}
