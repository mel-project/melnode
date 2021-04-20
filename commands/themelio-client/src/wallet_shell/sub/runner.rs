use crate::common::context::ExecutionContext;
use crate::common::executor::CommonCommandExecutor;
use crate::interactive::executor::InteractiveCommandExecutor;
use crate::interactive::sub::command::InteractiveSubCommand;
use crate::interactive::sub::input::{format_sub_prompt, read_line};
use crate::interactive::sub::output::{dispatch_error, exit, help, readline_error};
use crate::wallet::manager::WalletManager;

/// A sub-wallet_shell runner executed within the higher-level wallet_shell.
/// This wallet_shell unlocks a wallet, transacts with the network and shows balances.
pub(crate) struct WalletSubShellRunner {
    context: ExecutionContext,
    name: String,
    secret: String,
}

impl WalletSubShellRunner {
    /// Create a new sub wallet_shell runner if wallet exists and we can unlock & load with the provided secret.
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
        let prompt = format_sub_prompt(&self.context.version, &self.name).await?;

        loop {
            // Get command from user input.
            match read_line(&prompt).await {
                Ok(open_cmd) => {
                    // Exit if the user chooses to exit.
                    if open_cmd == InteractiveSubCommand::Exit {
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
    async fn dispatch(&self, interactive_cmd: &InteractiveSubCommand) -> anyhow::Result<()> {
        // Dispatch a command and return a command result
        match &interactive_cmd {
            InteractiveSubCommand::Faucet(amt, unit) => {
                self.faucet(amt, unit).await?;
            }
            InteractiveSubCommand::SendCoins(dest, amt, unit) => {
                self.send_coins(dest, amt, unit).await?;
            }
            InteractiveSubCommand::AddCoins(coin_id) => {
                self.add_coins(coin_id).await?;
            }
            InteractiveSubCommand::ShowBalance => {
                self.balance().await?;
            }
            InteractiveSubCommand::Help => {
                self.help().await?;
            }
            InteractiveSubCommand::Exit => {
                self.exit().await?;
            }
        }
        Ok(())
    }

    /// Calls faucet on the command executor with the inputs passed into sub-wallet_shell.
    async fn faucet(&self, amt: &str, denom: &str) -> anyhow::Result<()> {
        let executor = InteractiveCommandExecutor::new(self.context.clone());
        executor.faucet(&self.name, &self.secret, amt, denom).await
    }

    /// Calls send coins on the command executor with the inputs passed into the sub-wallet_shell.
    async fn send_coins(&self, dest: &str, amt: &str, unit: &str) -> anyhow::Result<()> {
        let executor = InteractiveCommandExecutor::new(self.context.clone());
        executor
            .send_coins(&self.name, &self.secret, dest, amt, unit)
            .await
    }

    /// Calls add coins on the command executor with the inputs passed into the sub-wallet_shell.
    async fn add_coins(&self, coin_id: &str) -> anyhow::Result<()> {
        let executor = InteractiveCommandExecutor::new(self.context.clone());
        executor.add_coins(&self.name, &self.secret, coin_id).await
    }

    /// Calls balance on the command executor with the inputs passed into the sub-wallet_shell.
    async fn balance(&self) -> anyhow::Result<()> {
        let executor = InteractiveCommandExecutor::new(self.context.clone());
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
