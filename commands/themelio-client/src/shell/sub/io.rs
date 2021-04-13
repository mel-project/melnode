use crate::common::read_line;
use crate::shell::sub::command::SubShellCommand;
use anyhow::Error;
use colored::Colorize;
use std::convert::TryFrom;

pub struct SubShellInput {}

impl SubShellInput {
    /// Format the CLI prompt with the version of the binary
    pub(crate) async fn format_prompt(version: &str, name: &str) -> anyhow::Result<String> {
        let prompt_stack: Vec<String> = vec![
            format!("themelio-client").cyan().bold().to_string(),
            format!("(v{})", version).magenta().to_string(),
            format!("➜ ").cyan().bold().to_string(),
            format!("(v{})", name).cyan().to_string(),
            format!("➜ ").cyan().bold().to_string(),
        ];
        Ok(format!("{}", prompt_stack.join(" ")))
    }

    /// Get user input and parse it into a shell command
    pub(crate) async fn command(prompt: &str) -> anyhow::Result<SubShellCommand> {
        let input = read_line(prompt.to_string()).await?;

        let open_wallet_cmd = SubShellCommand::try_from(input.to_string())?;
        Ok(open_wallet_cmd)
    }
}

pub struct SubShellOutput {}

impl SubShellOutput {

    /// Send coins to a recipient.
    pub(crate) async fn show_sent_coins() {}

    /// Add coins into wallet storage.
    pub(crate) async fn show_added_coins() {}

    /// Transfer coins from faucet to your wallet.
    pub(crate) async fn show_faucet_tx() {}

    /// Output the error when dispatching command
    pub(crate) async fn error(err: &Error, sub_shell_cmd: &SubShellCommand) -> anyhow::Result<()> {
        eprintln!("ERROR: {} when dispatching {:?}", err, sub_shell_cmd);
        Ok(())
    }

    /// Show available input commands
    pub(crate) async fn help() -> anyhow::Result<()> {
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
        Ok(())
    }

    /// Show exit message
    pub(crate) async fn exit() -> anyhow::Result<()> {
        eprintln!("\nExiting Themelio Client sub-shell");
        Ok(())
    }
}