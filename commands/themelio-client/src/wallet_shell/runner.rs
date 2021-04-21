use crate::common::context::ExecutionContext;
use crate::common::executor::CommandExecutor;
use crate::common::prompt::prompt::InputPrompt;
use crate::wallet_shell::command::ShellCommand;
use crate::wallet_shell::output::{command_error, exit, readline_error};
use crate::wallet_shell::prompt::ShellInputPrompt;
use crate::wallet_shell::sub::runner::WalletSubShellRunner;

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
        let prompt = ShellInputPrompt::new();
        let formatted_prompt = prompt.format_prompt(&self.context.version).await?;

        loop {
            let prompt_input = prompt.read_line(&formatted_prompt).await;

            // Get command from user input.
            match prompt_input {
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
    async fn dispatch(&self, cmd: &ShellCommand) -> anyhow::Result<()> {
        let ce = CommandExecutor::new(self.context.clone());
        // Dispatch a command and return a command result.
        match &cmd {
            ShellCommand::CreateWallet(name) => ce.create_wallet(name).await,
            ShellCommand::ShowWallets => ce.show_wallets().await,
            ShellCommand::OpenWallet(name, secret) => self.open_wallet(name, secret).await,
            ShellCommand::Help => self.help().await,
            ShellCommand::Exit => self.exit().await,
        }
    }

    async fn open_wallet(&self, name: &str, secret: &str) -> anyhow::Result<()> {
        let runner = WalletSubShellRunner::new(self.context.clone(), name, secret).await?;
        runner.run().await?;
        Ok(())
    }

    async fn help(&self) -> anyhow::Result<()> {
        eprintln!("\nAvailable commands are: ");
        eprintln!(">> create-wallet <wallet-name>");
        eprintln!(">> open-wallet <wallet-name> <secret>");
        eprintln!(">> show");
        eprintln!(">> help");
        eprintln!(">> exit");
        eprintln!(">> ");
        Ok(())
    }

    async fn exit(&self) -> anyhow::Result<()> {
        eprintln!("\nExiting Themelio Client wallet shell");
        Ok(())
    }
}
