use crate::shell::command::ShellCommand;
use crate::shell::io::{ShellInput, ShellOutput};
use crate::shell::sub::runner::SubShellRunner;
use crate::executor::CommandExecutor;
use crate::common::ExecutionContext;

/// Run an interactive shell given an execution context
/// This is for end users to create and show wallets
/// as well as open up a particular wallet to transact with network.
pub struct ShellRunner {
    context: ExecutionContext
}

impl ShellRunner {
    pub fn new(context: ExecutionContext) -> Self {
        Self { context }
    }

    /// Run shell commands from user input until user exits.
    pub async fn run(&self) -> anyhow::Result<()> {
        // Format user prompt.
        let prompt = ShellInput::format_prompt(&self.context.version).await?;

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

    /// Dispatch and process the shell command.
    async fn dispatch(&self, cmd: &ShellCommand) -> anyhow::Result<()> {
        let ce = CommandExecutor::new(self.context.clone());
        // Dispatch a command and return a command result.
        match &cmd {
            ShellCommand::CreateWallet(name) => ce.create_wallet(name).await,
            ShellCommand::ShowWallets => ce.show_wallets().await,
            ShellCommand::OpenWallet(name, secret) => self.open_wallet(name, secret).await,
            ShellCommand::Help => self.help().await,
            ShellCommand::Exit => { self.exit().await }
        }
    }

    /// Open a sub-shell given the name and secret and run in sub shell mode until user exits.
    async fn open_wallet(
        &self,
        name: &str,
        secret: &str,
    ) -> anyhow::Result<()> {
        let runner = SubShellRunner::new(self.context.clone(), name, secret).await?;
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
