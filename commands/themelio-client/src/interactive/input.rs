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

/// Format the interactive prompt with the version of the binary.
pub(crate) async fn format_interactive_prompt(version: &str) -> anyhow::Result<String> {
    let prompt_stack: Vec<String> = vec![
        format!("themelio-client").cyan().bold().to_string(),
        format!("(v{})", version).magenta().to_string(),
        format!("➜ ").cyan().bold().to_string(),
    ];
    Ok(format!("{}", prompt_stack.join(" ")))
}

/// Get user input and parse it into a interactive command.
pub(crate) async fn read_shell_input(prompt: &str) -> anyhow::Result<InteractiveCommand> {
    let input = read_line(prompt.to_string()).await?;
    let wallet_cmd = InteractiveCommand::try_from(input.to_string())?;
    Ok(wallet_cmd)
}
