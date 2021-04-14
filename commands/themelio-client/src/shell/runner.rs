use crate::shell::command::ShellCommand;
use crate::shell::io::{ShellInput, ShellOutput};
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

    /// Run shell commands from user input until user exits.
    pub async fn run(&self) -> anyhow::Result<()> {
        // Format user prompt.
        let prompt = ShellInput::format_prompt(&self.version).await?;

        loop {
            // Get command from user input.
            match ShellInput::read_line(&prompt).await {
                Ok(cmd) => {
                    // Exit if the user chooses to exit.
                    if cmd == ShellCommand::Exit {
                        ShellOutput::exit().await?;
                        return Ok(());
                    }

                    // Dispatch the command
                    let dispatch_result = &self.dispatch(&cmd).await;

                    // Output error, if any, and continue running.
                    match dispatch_result {
                        Err(err) => ShellOutput::shell_error(err, &cmd).await?,
                        _ => {}
                    }
                }
                Err(err) => {ShellOutput::readline_error(&err).await? }
            }
        }
    }

    /// Dispatch and process the command.
    async fn dispatch(&self, cmd: &ShellCommand) -> anyhow::Result<()> {
        // Dispatch a command and return a command result.
        match &cmd {
            ShellCommand::CreateWallet(name) => self.create(name).await,
            ShellCommand::ShowWallets => self.show().await,
            ShellCommand::OpenWallet(name, secret) => self.open(name, secret).await,
            ShellCommand::Help => self.help().await,
            ShellCommand::Exit => { self.exit().await }
        }
    }

    /// Create a new wallet and output it's information to user.
    async fn create(&self, name: &str) -> anyhow::Result<()> {
        // let wallet = WalletManager::new(&self.host, &self.database);
        // let (sk, wallet_data) = wallet.create_wallet(name).await?;
        // ShellOutput::(name, sk, &wallet_data);
        Ok(())
    }

    /// Shows all stored wallets.
    async fn show(&self) -> anyhow::Result<()> {
        // let wallet = WalletManager::new(&self.host, &self.database);
        // let wallets = wallet.get_all_wallets().await?;
        // ShellOutput::wallets(wallets).await;
        Ok(())
    }

    /// Open a sub-shell given the name and secret and run in sub shell mode until user exits.
    async fn open(
        &self,
        name: &str,
        secret: &str,
    ) -> anyhow::Result<()> {
        let runner = SubShellRunner::new(&self.host, &self.database, &self.version, name, secret).await?;
        runner.run().await?;
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
