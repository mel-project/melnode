use crate::utils::context::ExecutionContext;
use crate::utils::executor::CommandExecutor;
use crate::utils::prompt::InputPrompt;
use crate::wallet::manager::WalletManager;
use crate::shell::sub::command::SubShellCommand;
use crate::shell::sub::output::{dispatch_error, exit, help, readline_error};
use crate::shell::sub::prompt::SubShellInputPrompt;

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
        let context = context.clone();

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
        let prompt = SubShellInputPrompt::new();
        let formatted_prompt = prompt
            .format_named_prompt(&self.context.version, &self.name)
            .await?;

        loop {
            // Get command from user input.

            let prompt_input = prompt.read_line(&formatted_prompt).await;
            match prompt_input {
                Ok(open_cmd) => {
                    // Exit if the user chooses to exit.
                    if open_cmd == SubShellCommand::Exit {
                        exit().await?;
                        return Ok(());
                    }

                    // Dispatch the command.
                    // TODO: clean this up as the match following this seems non-canonical.
                    let dispatch_result = &self.dispatch(&open_cmd).await;

                    // Output error, if any, and continue running.
                    match dispatch_result {
                        Err(err) => dispatch_error(err, &open_cmd).await?,
                        _ => {}
                    }
                }
                Err(err) => readline_error(&err).await?,
            }
        }
    }

    /// Dispatch and process a single sub-wallet_shell command.
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
                self.help().await?;
            }
            SubShellCommand::Exit => {
                self.exit().await?;
            }
        }
        Ok(())
    }

    /// Calls faucet on the command executor with the inputs passed into sub-wallet_shell.
    async fn faucet(&self, amt: &str, denom: &str) -> anyhow::Result<()> {
        let executor = CommandExecutor::new(self.context.clone());
        executor.faucet(&self.name, &self.secret, amt, denom).await
    }

    /// Calls send coins on the command executor with the inputs passed into the sub-wallet_shell.
    async fn send_coins(&self, dest: &str, amt: &str, unit: &str) -> anyhow::Result<()> {
        let executor = CommandExecutor::new(self.context.clone());
        executor
            .send_coins(&self.name, &self.secret, dest, amt, unit)
            .await
    }

    /// Calls add coins on the command executor with the inputs passed into the sub-wallet_shell.
    async fn add_coins(&self, coin_id: &str) -> anyhow::Result<()> {
        let executor = CommandExecutor::new(self.context.clone());
        executor.add_coins(&self.name, &self.secret, coin_id).await
    }

    /// Calls balance on the command executor with the inputs passed into the sub-wallet_shell.
    async fn balance(&self) -> anyhow::Result<()> {
        let executor = CommandExecutor::new(self.context.clone());
        executor.show_balance(&self.name, &self.secret).await
    }

    /// Show available sub wallet_shell command args & inputs to user
    async fn help(&self) -> anyhow::Result<()> {
        help().await?;
        Ok(())
    }

    /// Show exit message
    async fn exit(&self) -> anyhow::Result<()> {
        exit().await?;
        Ok(())
    }
}
