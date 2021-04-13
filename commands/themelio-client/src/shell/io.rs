use crate::shell::command::ShellCommand;
use crate::shell::sub::command::SubShellCommand;
use crate::common::read_line;
use colored::Colorize;
use std::convert::TryFrom;
use anyhow::Error;

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
    pub(crate) async fn read_line(prompt: &str) -> anyhow::Result<(ShellCommand, Option<SubShellCommand>)> {
        let input = read_line(prompt.to_string()).await?;

        let wallet_use_mode: String = ShellCommand::UseWallet(String::default(), String::default())
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
    /// Output the error when dispatching command.
    pub(crate) async fn error(err: &Error, cmd: &ShellCommand) -> anyhow::Result<()> {
        eprintln!("ERROR: {} with shell command {:?}", err, cmd);
        Ok(())
    }

    /// Show available input commands.
    pub(crate) async fn help() -> anyhow::Result<()> {
        eprintln!("\nAvailable commands are: ");
        eprintln!(">> create <wallet-name>");
        eprintln!(">> open <wallet-name> <secret>");
        eprintln!(">> use <wallet-name> <secret> <args>");
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
