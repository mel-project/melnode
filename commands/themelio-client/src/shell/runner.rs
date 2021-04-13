use crate::shell::command::ShellCommand;
use crate::shell::sub::command::SubShellCommand;
use crate::wallet::wallet::Wallet;
use crate::shell::prompter::{ShellInput, ShellOutput};
use crate::shell::sub::runner::SubShellRunner;

pub struct ShellRunner {
    host: smol::net::SocketAddr,
    database: std::path::PathBuf,
    version: String
}

impl ShellRunner {
    pub fn new(host: &smol::net::SocketAddr, database: &std::path::PathBuf, version: &str) -> Self {
        let host = host.clone();
        let database = database.clone();
        let version = version.to_string();
        Self { host, database, version }
    }

    /// Dispatch commands from user input and show output using shell io prompter until user exits.
    pub async fn run(&self) -> anyhow::Result<()> {
        // Format user prompt.
        let prompt = ShellInput::format_prompt(&self.version).await?;

        loop {
            // Get command from user input.
            let (cmd, open_cmd) = ShellInput::read_line(&prompt).await?;

            // Exit if the user chooses to exit.
            if cmd == ShellCommand::Exit {
                ShellOutput::exit().await?;
                return Ok(());
            }

            // Dispatch the command (with an optional 'sub' command for 'use' single-line mode).
            let dispatch_result = &self.dispatch(&cmd, &open_cmd).await;

            // Output error, if any, and continue running.
            match dispatch_result {
                Err(err) => ShellOutput::error(err, &cmd).await?,
                _ => {}
            }
        }
    }

    /// Dispatch and process the command.
    async fn dispatch(&self, cmd: &ShellCommand, open_cmd: &Option<SubShellCommand>) -> anyhow::Result<()> {
        // Dispatch a command and return a command result.
        match &cmd {
            ShellCommand::CreateWallet(name) => self.create(name).await,
            ShellCommand::ShowWallets => self.show().await,
            ShellCommand::OpenWallet(name, secret) => self.open(name, secret).await,
            ShellCommand::UseWallet(name, secret) => self.use_(name, secret, &open_cmd).await,
            // ShellCommand::DeleteWallet(name) => self.delete(name).await,
            ShellCommand::Help => self.help().await,
            ShellCommand::Exit => { self.exit().await }
        }
    }

    /// Create a new shell and output it's information to user.
    async fn create(&self, name: &str) -> anyhow::Result<()> {
        let wallet = Wallet::new(&self.host, &self.database);
        let (sk, wallet_data) = wallet.create(name).await?;
        ShellOutput::(name, sk, &wallet_data);
        Ok(())
    }

    /// Delete an existing shell.
    async fn delete(&self, _name: &str) -> anyhow::Result<()> {
        todo!("Not implemented")
    }

    /// Shows all stored shell data.
    async fn show(&self) -> anyhow::Result<()> {
        let wallet = Wallet::new(&self.host, &self.database);
        let wallets = wallet.get_all().await?;
        ShellOutput::wallets(wallets).await;
        Ok(())
    }

    /// Open a shell given the name and secret and run in sub shell dispatch mode.
    async fn open(
        &self,
        name: &str,
        secret: &str,
    ) -> anyhow::Result<()> {
        let executor = SubShellRunner::new(&self.host, &self.database, &self.version, name, secret).await?;
        executor.run().await?;
        Ok(())
    }

    /// Use a particular shell to dispatch a single sub shell command.
    async fn use_(
        &self,
        name: &str,
        secret: &str,
        open_wallet_command: &Option<SubShellCommand>,
    ) -> anyhow::Result<()> {
        let executor = SubShellRunner::new(&self.host, &self.database, &self.version, name, secret).await?;
        executor.run_once(&open_wallet_command.clone().unwrap()).await?;
        Ok(())
    }

    /// Output help message to user.
    async fn help(&self) -> anyhow::Result<()> {
        ShellOutput::help().await?;
        Ok(())
    }

    /// Output exit message to user.
    async fn exit(&self) -> anyhow::Result<()> {
        ShellOutput::exit().await?;
        Ok(())
    }
}
