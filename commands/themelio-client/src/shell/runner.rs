use crate::shell::command::ShellCommand;
use crate::shell::output::{print_command_error, print_exit_message, print_readline_error, print_shell_help, print_shell_exit};
use crate::utils::context::ExecutionContext;
use crate::utils::executor::CommandExecutor;
use crate::shell::sub_runner::WalletSubShellRunner;
use crate::shell::prompt::{format_prompt, read_shell_command};

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
        let formatted_prompt = format_prompt(&self.context.version);

        loop {
            let prompt_input = read_shell_command(&formatted_prompt).await;

            // Get command from user input.
            match prompt_input {
                Ok(cmd) => {
                    // Exit if the user chooses to exit.
                    if cmd == ShellCommand::Exit {
                        print_exit_message();
                        return Ok(());
                    }

                    // Output error, if any, and continue running.
                    if let Err(err) = self.dispatch(&cmd).await {
                        print_command_error(&err, &cmd)
                    }
                }
                // Output parsing error and continue running.
                Err(err) => print_readline_error(&err),
            }
        }
    }
    async fn dispatch(&self, cmd: &ShellCommand) -> anyhow::Result<()> {
        let ce =CommandExecutor::new(self.context.clone());

        match &cmd {
            ShellCommand::CreateWallet(wallet_name) => {
                ce.create_wallet(wallet_name).await?;
            }
            ShellCommand::ShowWallets => {
                ce.show_wallets().await?;
            }
            ShellCommand::OpenWallet(wallet_name, secret) => {
                self.open_wallet(wallet_name, secret).await?;
            }
            ShellCommand::Help => { print_shell_help(); }
            ShellCommand::Exit => { print_shell_exit(); }
        }
        Ok(())
    }

    async fn open_wallet(&self, name: &str, secret: &str) -> anyhow::Result<()> {
        let runner = WalletSubShellRunner::new(self.context.clone(), name, secret).await?;
        runner.run().await?;
        Ok(())
    }

}
