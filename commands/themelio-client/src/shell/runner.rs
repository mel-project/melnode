use crate::context::ExecutionContext;
use crate::executor::CommandExecutor;
use crate::shell::command::ShellCommand;
use crate::shell::io::common_read_line;
use crate::shell::io::print_readline_error;
use crate::shell::sub_runner::WalletSubShellRunner;
use crate::wallet::error::WalletError;
use std::convert::TryFrom;
use std::fmt::Error;

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
        let formatted_prompt = self.format_prompt();

        loop {
            let prompt_input = self.read_command(&formatted_prompt).await;

            // Get command from user input.
            match prompt_input {
                Ok(cmd) => {
                    // Exit if the user chooses to exit.
                    if cmd == ShellCommand::Exit {
                        self.exit();
                        return Ok(());
                    }

                    // Output error, if any, and continue running.
                    if let Err(err) = self.dispatch(&cmd).await {
                        self.print_command_error(&err, &cmd)
                    }
                }
                // Output parsing error and continue running.
                Err(err) => print_readline_error(&err),
            }
        }
    }
    async fn dispatch(&self, cmd: &ShellCommand) -> anyhow::Result<()> {
        let ce = CommandExecutor::new(self.context.clone());

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
            ShellCommand::Help => {
                self.help();
            }
            ShellCommand::Exit => {
                self.exit();
            }
        }
        Ok(())
    }

    async fn open_wallet(&self, name: &str, secret: &str) -> anyhow::Result<()> {
        let runner = WalletSubShellRunner::new(self.context.clone(), name, secret).await?;
        runner.run().await?;
        Ok(())
    }

    /// Show exit message.
    fn exit(&self) {
        eprintln!("\nExiting Themelio Client wallet_shell");
    }

    /// Show available input commands for the shell
    fn help(&self) {
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
        let input = common_read_line(prompt.to_string()).await?;
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
