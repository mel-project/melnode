use crate::shell::command::ShellCommand;
use crate::common::read_line;
use colored::Colorize;
use std::convert::TryFrom;
use anyhow::Error;
use crate::wallet::data::WalletData;
use tabwriter::TabWriter;

use std::io::prelude::*;
use std::collections::BTreeMap;
use crate::wallet::wallet::Wallet;

pub struct ShellInput {}

impl ShellInput {
    /// Format the CLI prompt with the version of the binary.
    pub(crate) async fn format_prompt(version: &str) -> anyhow::Result<String> {
        let prompt_stack: Vec<String> = vec![
            format!("themelio-client").cyan().bold().to_string(),
            format!("(v{})", version).magenta().to_string(),
            format!("➜ ").cyan().bold().to_string(),
        ];
        Ok(format!("{}", prompt_stack.join(" ")))
    }

    /// Get user input and parse it into a shell command.
    pub(crate) async fn read_line(prompt: &str) -> anyhow::Result<ShellCommand> {
        let input = read_line(prompt.to_string()).await?;
        let wallet_cmd = ShellCommand::try_from(input.to_string())?;
        Ok(wallet_cmd)
    }
}

pub struct ShellOutput {}

impl ShellOutput {
    /// Display name, secret key and covenant of the wallet.
    pub(crate) async fn show_new_wallet(wallet: Wallet) -> anyhow::Result<()> {
        let mut tw = TabWriter::new(vec![]);
        writeln!(tw, ">> New data:\t{}", wallet.name.bold()).unwrap();
        writeln!(tw, ">> Address:\t{}", wallet.data.my_script.hash().to_addr().yellow()).unwrap();
        writeln!(tw, ">> Secret:\t{}", hex::encode(wallet.sk.0).dimmed()).unwrap();
        eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
        Ok(())
    }

    /// Display all stored wallet wallet addresses by name.
    pub(crate) async fn show_all_wallets(wallets: BTreeMap<String, WalletData>) {
        let mut tw = TabWriter::new(vec![]);
        writeln!(tw, ">> [NAME]\t[ADDRESS]");
        for (name, wallet) in wallets {
            writeln!(tw, ">> {}\t{}", name, wallet.my_script.hash().to_addr());
        }
        tw.flush();
        eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
    }

    /// Output the error when dispatching command.
    pub(crate) async fn shell_error(err: &Error, cmd: &ShellCommand) -> anyhow::Result<()> {
        eprintln!("ERROR: {} with shell command {:?}", err, cmd);
        Ok(())
    }

    /// Output the error when reading user input.
    pub(crate) async fn readline_error(_err: &Error) -> anyhow::Result<()> {
        eprintln!("ERROR: can't parse input command");
        Ok(())
    }

    /// Show available input commands.
    pub(crate) async fn help() -> anyhow::Result<()> {
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
        eprintln!("\nExiting Themelio Client shell");
        Ok(())
    }
}
