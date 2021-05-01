use crate::context::ExecutionContext;
use crate::executor::CommandExecutor;
use crate::shell::command::ShellCommand;
use crate::shell::common::print_error;
use crate::shell::common::read_line;
use crate::shell::sub_runner::WalletSubShellRunner;
use crate::wallet::error::WalletError;
use crate::wallet::info::Printable;
use anyhow::Error;
use colored::Colorize;
use std::convert::TryFrom;

/// Run an wallet_shell command given an execution context
/// This is for end users to create and show wallets
/// as well as open up a particular wallet to transact with network.
pub struct WalletShellRunner {
    context: ExecutionContext,
}

impl WalletShellRunner {
    pub(crate) fn new(context: ExecutionContext) -> Self {
        Self { context }
    }

    /// Run wallet_shell commands from user input until user exits.
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        // Format user prompt.
        let formatted_prompt = self.format_prompt();

        loop {
            // Get command from user input.
            let prompt_input = self.read_command(&formatted_prompt).await;

            match prompt_input {
                Ok(shell_cmd) => {
                    // Exit if the user chooses to exit.
                    if shell_cmd == ShellCommand::Exit {
                        self.print_exit();
                        return Ok(());
                    }

                    // Output error, if any, and continue running.
                    if let Err(err) = self.dispatch(&shell_cmd).await {
                        self.print_command_error(&err, &shell_cmd)
                    }
                }
                // Output parsing error and continue running.
                Err(err) => print_error(&err),
            }
        }
    }

    /// Dispatch and process a single shell command.
    async fn dispatch(&self, cmd: &ShellCommand) -> anyhow::Result<()> {
        let ce = CommandExecutor::new(self.context.clone());

        match &cmd {
            ShellCommand::CreateWallet(wallet_name) => {
                ce.create_wallet(wallet_name).await?;
            }
            ShellCommand::ShowWallets => {
                let info = ce.show_wallets().await?;
                info.print(&mut std::io::stderr());
            }
            ShellCommand::OpenWallet(wallet_name, secret) => {
                self.open_wallet(wallet_name, secret).await?;
            }
            ShellCommand::Help => {
                self.print_help();
            }
            ShellCommand::Exit => {
                self.print_exit();
            }
        }
        Ok(())
    }

    /// Start the sub shell runner with a particular wallet
    async fn open_wallet(&self, name: &str, secret: &str) -> anyhow::Result<()> {
        let runner = WalletSubShellRunner::new(self.context.clone(), name, secret).await?;
        runner.run().await?;
        Ok(())
    }

    /// Show exit message.
    fn print_exit(&self) {
        eprintln!("\nExiting Themelio Client wallet_shell");
    }

    /// Show available input commands for the shell
    fn print_help(&self) {
        eprintln!("\nAvailable commands are: ");
        eprintln!(">> create-wallet <wallet-name>");
        eprintln!(">> open-wallet <wallet-name> <secret>");
        eprintln!(">> show-wallets");
        eprintln!(">> help");
        eprintln!(">> exit");
        eprintln!(">> ");
    }

    /// Read input to prompt and parse it into a shell command
    pub(crate) async fn read_command(&self, prompt: &str) -> anyhow::Result<ShellCommand> {
        let input = read_line(prompt.to_string()).await?;
        let wallet_cmd = ShellCommand::try_from(input.clone());
        if wallet_cmd.is_err() {
            anyhow::bail!(WalletError::InvalidInputArgs(input))
        }
        Ok(wallet_cmd?)
    }

    /// Output the error when dispatching command.
    fn print_command_error(&self, err: &Error, cmd: &ShellCommand) {
        eprintln!("ERROR: {} with wallet_shell command {:?}", err, cmd);
    }

    /// Create a prompt for shell mode
    fn format_prompt(&self) -> String {
        let version = self.context.version.clone();
        let prompt_stack: Vec<String> = vec![
            "themelio-client".to_string().cyan().bold().to_string(),
            format!("(v{})", &version).magenta().to_string(),
            "➜ ".to_string().cyan().bold().to_string(),
        ];
        prompt_stack.join(" ")
    }
}
