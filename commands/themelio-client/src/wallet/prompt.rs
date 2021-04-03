use crate::wallet::command::{WalletCommand, WalletCommandResult};
use crate::wallet::open::command::OpenWalletCommand;
use crate::wallet::common::read_line;
use colored::Colorize;
use std::convert::TryFrom;
use anyhow::Error;

pub struct WalletPrompt {
    prompt: String
}

impl WalletPrompt {
    pub fn new(version: &str) -> Self {
        let prompt_stack: Vec<String> = vec![
            format!("themelio-client").cyan().bold().to_string(),
            format!("(v{})", version).magenta().to_string(),
            format!("➜ ").cyan().bold().to_string(),
        ];
        let prompt = format!("{}", prompt_stack.join(" "));
        Self {
            prompt
        }
    }

    /// Given the user input parse it into a wallet and (if applicable) open wallet command
    pub(crate) async fn input(&self) -> anyhow::Result<(WalletCommand, Option<OpenWalletCommand>)> {
        let input = read_line(self.prompt.clone()).await?;

        let wallet_use_mode: String = WalletCommand::Use(String::default(), String::default())
            .to_string()
            .split(" ")
            .map(|s|s.to_string())
            .next()
            .unwrap();

        if input.starts_with(&wallet_use_mode) {
            let args: Vec<String> = input.split(" ").map(|s| s.to_string()).collect();
            let (left, right): (&str, &str) = (&args[0..2].join(" "), &args[2..].join(" "));
            let wallet_cmd = WalletCommand::try_from(left.to_string())?;
            let open_wallet_cmd = OpenWalletCommand::try_from(right.to_string())?;
            Ok((wallet_cmd, Some(open_wallet_cmd)))
        } else {
            let wallet_cmd = WalletCommand::try_from(input.to_string())?;
            Ok((wallet_cmd, None))
        }
    }

    pub(crate) async fn output(&self, cmd_result: &WalletCommandResult) -> anyhow::Result<()> {
        Ok(())
    }

    pub(crate) async fn error(&self, err: &Error, wallet_cmd: &WalletCommand) -> anyhow::Result<()> {
        eprintln!("ERROR: {} when dispatching {:?}", err, wallet_cmd);
        Ok(())
    }
}
