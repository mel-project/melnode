use crate::common::context::ExecutionContext;
use crate::common::executor::CommonCommandExecutor;
use crate::wallet_shell::input::{format_prompt, read_line};
use crate::wallet_shell::command::ShellCommand;
use crate::wallet_shell::output::exit;

/// Run an wallet_shell command given an execution context
/// This is for end users to create and show wallets
/// as well as open up a particular wallet to transact with network.
pub struct WalletShellRunner {
    context: ExecutionContext,
}

impl WalletShellRunner {
    pub fn new(context: ExecutionContext) -> Self {
        Self { context }
    }

    /// Run wallet_shell commands from user input until user exits.
    pub async fn run(&self) -> anyhow::Result<()> {
        // Format user prompt.
        let prompt = format_prompt(&self.context.version).await?;

        loop {
            // Get command from user input.
            match read_line(&prompt).await {
                Ok(cmd) => {
                    // Exit if the user chooses to exit.
                    if cmd == ShellCommand::Exit {
                        exit().await?;
                        return Ok(());
                    }

                    // Dispatch the command
                    let dispatch_result = &self.dispatch(&cmd).await;

                    // Output error, if any, and continue running.
                    match dispatch_result {
                        Err(err) => command_error(err, &cmd).await?,
                        _ => {}
                    }
                }
                Err(err) => readline_error(&err).await?,
            }
        }
    }

    /// Dispatch and process the wallet_shell command.
    async fn dispatch(&self, cmd: &InteractiveCommand) -> anyhow::Result<()> {
        let ce = CommonCommandExecutor::new(self.context.clone());
        // Dispatch a command and return a command result.
        match &cmd {
            InteractiveCommand::CreateWallet(name) => ce.create_wallet(name).await,
            InteractiveCommand::ShowWallets => ce.show_wallets().await,
            InteractiveCommand::OpenWallet(name, secret) => self.open_wallet(name, secret).await,
            InteractiveCommand::Help => self.help().await,
            InteractiveCommand::Exit => self.exit().await,
        }
    }

    /// Open a sub-wallet_shell given the name and secret and run in sub wallet_shell mode until user exits.
    async fn open_wallet(&self, name: &str, secret: &str) -> anyhow::Result<()> {
        let runner = InteractiveSubCommandRunner::new(self.context.clone(), name, secret).await?;
        runner.run().await?;
        Ok(())
    }

    /// Output help message to user.
    async fn help(&self) -> anyhow::Result<()> {
        help().await?;
        Ok(())
    }

    /// Output exit message to user.
    async fn exit(&self) -> anyhow::Result<()> {
        exit().await?;
        Ok(())
    }
}
