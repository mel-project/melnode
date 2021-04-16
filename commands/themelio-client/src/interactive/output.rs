use crate::common::read_line;
use crate::interactive::command::InteractiveCommand;
use crate::wallet::data::WalletData;
use anyhow::Error;
use colored::Colorize;
use std::convert::TryFrom;
use tabwriter::TabWriter;

use crate::wallet::wallet::Wallet;
use std::collections::BTreeMap;
use std::io::prelude::*;

/// Output the error when dispatching command.
pub(crate) async fn shell_error(err: &Error, cmd: &InteractiveCommand) -> anyhow::Result<()> {
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
