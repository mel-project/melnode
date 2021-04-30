use crate::context::ExecutionContext;
use crate::executor::CommandExecutor;
use crate::shell::command::SubShellCommand;
use crate::shell::common::{print_error, read_line};
use crate::wallet::manager::WalletManager;
use anyhow::Error;
use colored::Colorize;
use std::convert::TryFrom;

/// A sub-wallet_shell runner executed within the higher-level wallet_shell.
/// This wallet_shell unlocks a wallet, transacts with the network and shows balances.
pub(crate) struct WalletSubShellRunner {
    context: ExecutionContext,
    name: String,
    secret: String,
}

impl WalletSubShellRunner {
    /// Create a new sub shell runner if wallet exists and we can unlock & load with the provided secret.
    pub(crate) async fn new(
        context: ExecutionContext,
        name: &str,
        secret: &str,
    ) -> anyhow::Result<Self> {
        let name = name.to_string();
        let secret = secret.to_string();

        // Ensure we can load & unlock this wallet with the seed.
        let manager = WalletManager::new(context.clone());
        let _ = manager.load_wallet(&name, &secret).await?;

        Ok(Self {
            context,
            name,
            secret,
        })
    }

    /// Read and execute sub-wallet_shell commands from user until user exits.
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        // Format user prompt.
        let formatted_prompt = self.format_prompt();

        loop {
            // Get command from user input.
            let prompt_input = self.read_command(&formatted_prompt).await;

            match prompt_input {
                Ok(sub_shell_cmd) => {
                    // Exit if the user chooses to exit.
                    if sub_shell_cmd == SubShellCommand::Exit {
                        self.print_exit();
                        return Ok(());
                    }

                    // Output error, if any, and continue running.
                    if let Err(err) = self.dispatch(&sub_shell_cmd).await {
                        self.print_command_error(&err, &sub_shell_cmd)
                    }
                }
                Err(err) => print_error(&err),
            }
        }
    }

    /// Dispatch and process a single sub shell command.
    async fn dispatch(&self, interactive_cmd: &SubShellCommand) -> anyhow::Result<()> {
        // Dispatch a command and return a command result
        match &interactive_cmd {
            SubShellCommand::Faucet(amt, unit) => {
                self.faucet(amt, unit).await?;
            }
            SubShellCommand::SendCoins(dest, amt, unit) => {
                self.send_coins(dest, amt, unit).await?;
            }
            SubShellCommand::AddCoins(coin_id) => {
                self.add_coins(coin_id).await?;
            }
            SubShellCommand::ShowBalance => {
                self.balance().await?;
            }
            SubShellCommand::Help => {
                self.print_help();
            }
            SubShellCommand::Exit => {
                self.print_exit();
            }
        }
        Ok(())
    }

    /// Calls faucet on the command executor with the inputs passed into sub-wallet_shell.
    async fn faucet(&self, amt: &str, denom: &str) -> anyhow::Result<()> {
        let executor = CommandExecutor::new(self.context.clone());
        let _ = executor
            .faucet(&self.name, &self.secret, amt, denom)
            .await?;
        Ok(())
    }

    /// Calls send coins on the command executor with the inputs passed into the sub-wallet_shell.
    async fn send_coins(&self, dest: &str, amt: &str, unit: &str) -> anyhow::Result<()> {
        let executor = CommandExecutor::new(self.context.clone());
        executor
            .send_coins(&self.name, &self.secret, dest, amt, unit)
            .await?;
        Ok(())
    }

    /// Calls add coins on the command executor with the inputs passed into the sub-wallet_shell.
    async fn add_coins(&self, coin_id: &str) -> anyhow::Result<()> {
        let executor = CommandExecutor::new(self.context.clone());
        executor
            .add_coins(&self.name, &self.secret, coin_id)
            .await?;
        Ok(())
    }

    /// Calls balance on the command executor with the inputs passed into the sub-wallet_shell.
    async fn balance(&self) -> anyhow::Result<()> {
        let executor = CommandExecutor::new(self.context.clone());
        executor.show_balance(&self.name, &self.secret).await?;
        Ok(())
    }

    /// Read input to prompt and parse it into a sub shell command
    async fn read_command(&self, prompt: &str) -> anyhow::Result<SubShellCommand> {
        let input = read_line(prompt.to_string()).await?;
        let open_wallet_cmd = SubShellCommand::try_from(input)?;
        Ok(open_wallet_cmd)
    }

    /// Show available input commands for the sub shell
    fn print_help(&self) {
        eprintln!("\nAvailable commands are: ");
        eprintln!(">> faucet <amount> <unit>");
        eprintln!(">> send-coins <address> <amount> <unit>");
        eprintln!(">> add-coins <coin-id>");
        // eprintln!(">> deposit args");
        // eprintln!(">> swap args");
        // eprintln!(">> withdraw args");
        eprintln!(">> balance");
        eprintln!(">> help");
        eprintln!(">> exit");
        eprintln!(">> ");
    }

    /// Show exit message
    fn print_exit(&self) {
        eprintln!("\nExiting Themelio Client active wallet");
    }

    /// Create a named prompt for sub shell mode to show wallet name.
    fn format_prompt(&self) -> String {
        let version = self.context.version.clone();
        let name = self.name.clone();
        let prompt_stack: Vec<String> = vec![
            "themelio-client".to_string().cyan().bold().to_string(),
            format!("(v{})", &version).magenta().to_string(),
            "➜ ".to_string().cyan().bold().to_string(),
            format!("({})", &name).cyan().to_string(),
            "➜ ".to_string().cyan().bold().to_string(),
        ];
        prompt_stack.join(" ")
    }

    /// Output the error when dispatching command
    fn print_command_error(&self, err: &Error, sub_cmd: &SubShellCommand) {
        eprintln!("ERROR: {} when dispatching {:?}", err, sub_cmd);
    }
}
