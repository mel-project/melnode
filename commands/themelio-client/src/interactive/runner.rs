use crate::common::ExecutionContext;
use crate::executor::CommandExecutor;
use crate::interactive::command::InteractiveCommand;
use crate::interactive::io::{InteractiveInput, InteractiveOutput};
use crate::interactive::sub::runner::SubShellRunner;
use crate::common::context::ExecutionContext;

/// Run an interactive command given an execution context
/// This is for end users to create and show wallets
/// as well as open up a particular wallet to transact with network.
pub struct InteractiveCommandRunner {
    context: ExecutionContext,
}

impl InteractiveCommandRunner {
    pub fn new(context: ExecutionContext) -> Self {
        Self { context }
    }

    /// Run interactive commands from user input until user exits.
    pub async fn run(&self) -> anyhow::Result<()> {
        // Format user prompt.
        let prompt = InteractiveInput::format_interactive_prompt(&self.context.version).await?;

        loop {
            // Get command from user input.
            match InteractiveInput::read_shell_input(&prompt).await {
                Ok(cmd) => {
                    // Exit if the user chooses to exit.
                    if cmd == InteractiveCommand::Exit {
                        InteractiveOutput::exit().await?;
                        return Ok(());
                    }

                    // Dispatch the command
                    let dispatch_result = &self.dispatch(&cmd).await;

                    // Output error, if any, and continue running.
                    match dispatch_result {
                        Err(err) => InteractiveOutput::shell_error(err, &cmd).await?,
                        _ => {}
                    }
                }
                Err(err) => InteractiveOutput::readline_error(&err).await?,
            }
        }
    }

    /// Dispatch and process the interactive command.
    async fn dispatch(&self, cmd: &InteractiveCommand) -> anyhow::Result<()> {
        let ce = CommandExecutor::new(self.context.clone());
        // Dispatch a command and return a command result.
        match &cmd {
            InteractiveCommand::CreateWallet(name) => ce.create_wallet(name).await,
            InteractiveCommand::ShowWallets => ce.show_wallets().await,
            InteractiveCommand::OpenWallet(name, secret) => self.open_wallet(name, secret).await,
            InteractiveCommand::Help => self.help().await,
            InteractiveCommand::Exit => self.exit().await,
        }
    }

    /// Open a sub-interactive given the name and secret and run in sub interactive mode until user exits.
    async fn open_wallet(&self, name: &str, secret: &str) -> anyhow::Result<()> {
        let runner = SubShellRunner::new(self.context.clone(), name, secret).await?;
        runner.run().await?;
        Ok(())
    }

    /// Output help message to user.
    async fn help(&self) -> anyhow::Result<()> {
        InteractiveOutput::shell_help().await?;
        Ok(())
    }

    /// Output exit message to user.
    async fn exit(&self) -> anyhow::Result<()> {
        InteractiveOutput::exit().await?;
        Ok(())
    }
}
