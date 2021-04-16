use crate::common::ExecutionContext;
use crate::executor::CommandExecutor;
use crate::interactive::sub::command::InteractiveSubCommand;
use crate::interactive::sub::io::{SubShellInput, SubShellOutput};
use crate::wallet::manager::WalletManager;
use crate::common::context::ExecutionContext;

/// A sub-interactive runner executed within the higher-level interactive.
/// This interactive unlocks a wallet, transacts with the network and shows balances.
pub(crate) struct SubShellRunner {
    context: ExecutionContext,
    name: String,
    secret: String,
}

impl SubShellRunner {
    /// Create a new sub interactive runner if wallet exists and we can unlock & load with the provided secret.
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

    /// Read and execute sub-interactive commands from user until user exits.
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        // Format user prompt.
        let prompt = SubShellInput::format_prompt(&self.context.version, &self.name).await?;

        loop {
            // Get command from user input.
            match SubShellInput::command(&prompt).await {
                Ok(open_cmd) => {
                    // Exit if the user chooses to exit.
                    if open_cmd == InteractiveSubCommand::Exit {
                        SubShellOutput::exit().await?;
                        return Ok(());
                    }

                    // Dispatch the command.
                    // TODO: clean this up as the match following this seems non-canonical.
                    let dispatch_result = &self.dispatch(&open_cmd).await;

                    // Output error, if any, and continue running.
                    match dispatch_result {
                        Err(err) => SubShellOutput::subshell_error(err, &open_cmd).await?,
                        _ => {}
                    }
                }
                Err(err) => SubShellOutput::readline_error(&err).await?,
            }
        }
    }

    /// Dispatch and process a single sub-interactive command.
    async fn dispatch(&self, sub_shell_cmd: &InteractiveSubCommand) -> anyhow::Result<()> {
        // Dispatch a command and return a command result
        match &sub_shell_cmd {
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

    /// Calls faucet on the command executor with the inputs passed into sub-interactive.
    async fn faucet(&self, amt: &str, denom: &str) -> anyhow::Result<()> {
        let executor = CommandExecutor::new(self.context.clone());
        executor.faucet(&self.name, &self.secret, amt, denom).await
    }

    /// Calls send coins on the command executor with the inputs passed into the sub-interactive.
    async fn send_coins(&self, dest: &str, amt: &str, unit: &str) -> anyhow::Result<()> {
        let executor = CommandExecutor::new(self.context.clone());
        executor
            .send_coins(&self.name, &self.secret, dest, amt, unit)
            .await
    }

    /// Calls add coins on the command executor with the inputs passed into the sub-interactive.
    async fn add_coins(&self, coin_id: &str) -> anyhow::Result<()> {
        let executor = CommandExecutor::new(self.context.clone());
        executor.add_coins(&self.name, &self.secret, coin_id).await
    }

    /// Calls balance on the command executor with the inputs passed into the sub-interactive.
    async fn balance(&self) -> anyhow::Result<()> {
        let executor = CommandExecutor::new(self.context.clone());
        executor.show_balance(&self.name, &self.secret).await
    }

    /// Show available sub interactive command args & inputs to user
    async fn help(&self) -> anyhow::Result<()> {
        // SubShellOutput::output_help().await?;
        Ok(())
    }

    /// Show exit message
    async fn exit(&self) -> anyhow::Result<()> {
        // SubShellOutput::output_help().await?;
        Ok(())
    }
}
