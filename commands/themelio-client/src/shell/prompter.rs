use crate::shell::command::ShellCommand;
use crate::shell::sub::command::SubShellCommand;
use crate::common::read_line;
use colored::Colorize;
use std::convert::TryFrom;
use anyhow::Error;
use tabwriter::TabWriter;
use tmelcrypt::Ed25519SK;
use crate::data::WalletData;

use std::io::prelude::*;
use std::collections::BTreeMap;

pub struct ShellInput {}

impl ShellInput {
    /// Format the CLI prompt with the version of the binary
    pub(crate) async fn format_prompt(version: &str) -> anyhow::Result<String> {
        let prompt_stack: Vec<String> = vec![
            format!("themelio-client").cyan().bold().to_string(),
            format!("(v{})", version).magenta().to_string(),
            format!("➜ ").cyan().bold().to_string(),
        ];
        Ok(format!("{}", prompt_stack.join(" ")))
    }

    /// Get user input and parse it into a shell command
    pub(crate) async fn command(prompt: &str) -> anyhow::Result<(ShellCommand, Option<SubShellCommand>)> {
        let input = read_line(prompt.to_string()).await?;

        let wallet_use_mode: String = ShellCommand::Use(String::default(), String::default())
            .to_string()
            .split(" ")
            .map(|s|s.to_string())
            .next()
            .unwrap();

        if input.starts_with(&wallet_use_mode) {
            let args: Vec<String> = input.split(" ").map(|s| s.to_string()).collect();
            let (left, right): (&str, &str) = (&args[0..2].join(" "), &args[2..].join(" "));
            let wallet_cmd = ShellCommand::try_from(left.to_string())?;
            let open_wallet_cmd = SubShellCommand::try_from(right.to_string())?;
            Ok((wallet_cmd, Some(open_wallet_cmd)))
        } else {
            let wallet_cmd = ShellCommand::try_from(input.to_string())?;
            Ok((wallet_cmd, None))
        }
    }
}

pub struct ShellOutput {}

impl ShellOutput {
    /// Display name, secret key and covenant of the shell
    pub(crate) async fn wallet(name: &str, sk: Ed25519SK, wallet_data: &WalletData) -> anyhow::Result<()>{
        // Display contents of keypair and address from covenant
        let mut tw = TabWriter::new(vec![]);
        writeln!(tw, ">> New data:\t{}", name.bold()).unwrap();
        writeln!(tw, ">> Address:\t{}", wallet_data.my_script.hash().to_addr().yellow()).unwrap();
        writeln!(tw, ">> Secret:\t{}", hex::encode(sk.0).dimmed()).unwrap();
        eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
        Ok(())
    }

    pub(crate) async fn wallets(wallets: BTreeMap<String, WalletData>) {
        let mut tw = TabWriter::new(vec![]);
        writeln!(tw, ">> [NAME]\t[ADDRESS]");
        for (name, wallet) in wallets {
            writeln!(tw, ">> {}\t{}", name, wallet.my_script.hash().to_addr());
        }
        tw.flush();
        eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
    }

    /// Output the error when dispatching command
    pub(crate) async fn error(err: &Error, wallet_cmd: &ShellCommand) -> anyhow::Result<()> {
        eprintln!("ERROR: {} when dispatching {:?}", err, wallet_cmd);
        Ok(())
    }

    /// Show available input commands
    pub(crate) async fn help() -> anyhow::Result<()> {
        eprintln!("\nAvailable commands are: ");
        eprintln!(">> create <shell-name>");
        eprintln!(">> sub <shell-name> <secret>");
        eprintln!(">> use <shell-name> <secret> <sub-shell-args>");
        eprintln!(">> show");
        eprintln!(">> help");
        eprintln!(">> exit");
        eprintln!(">> ");
        Ok(())
    }

    /// Show exit message
    pub(crate) async fn exit() -> anyhow::Result<()> {
        eprintln!("\nExiting Themelio Client");
        Ok(())
    }
}
