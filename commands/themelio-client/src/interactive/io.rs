use crate::common::read_line;
use crate::interactive::command::ShellCommand;
use crate::wallet::data::WalletData;
use anyhow::Error;
use colored::Colorize;
use std::convert::TryFrom;
use tabwriter::TabWriter;

use crate::wallet::wallet::Wallet;
use std::collections::BTreeMap;
use std::io::prelude::*;

pub struct ShellInput {}

impl ShellInput {
    /// Format the interactive prompt with the version of the binary.
    pub(crate) async fn format_shell_prompt(version: &str) -> anyhow::Result<String> {
        let prompt_stack: Vec<String> = vec![
            format!("themelio-client").cyan().bold().to_string(),
            format!("(v{})", version).magenta().to_string(),
            format!("➜ ").cyan().bold().to_string(),
        ];
        Ok(format!("{}", prompt_stack.join(" ")))
    }

    /// Get user input and parse it into a interactive command.
    pub(crate) async fn read_shell_input(prompt: &str) -> anyhow::Result<ShellCommand> {
        let input = read_line(prompt.to_string()).await?;
        let wallet_cmd = ShellCommand::try_from(input.to_string())?;
        Ok(wallet_cmd)
    }
}

pub struct ShellOutput {}

impl ShellOutput {
    /// Output the error when dispatching command.
    pub(crate) async fn shell_error(err: &Error, cmd: &ShellCommand) -> anyhow::Result<()> {
        eprintln!("ERROR: {} with interactive command {:?}", err, cmd);
        Ok(())
    }

    /// Output the error when reading user input.
    pub(crate) async fn readline_error(_err: &Error) -> anyhow::Result<()> {
        eprintln!("ERROR: can't parse input command");
        Ok(())
    }

    /// Show available input commands.
    pub(crate) async fn shell_help() -> anyhow::Result<()> {
        eprintln!("\nAvailable commands are: ");
        eprintln!(">> create-wallet <wallet-name>");
        eprintln!(">> open-wallet <wallet-name> <secret>");
        eprintln!(">> show");
        eprintln!(">> help");
        eprintln!(">> exit");
        eprintln!(">> ");
        Ok(())
    }

    /// Show exit message.
    pub(crate) async fn exit() -> anyhow::Result<()> {
        eprintln!("\nExiting Themelio Client interactive");
        Ok(())
    }
}
