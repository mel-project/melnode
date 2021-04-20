use std::convert::TryFrom;

use colored::Colorize;

use crate::common::input::read_line as common_read_line;
use crate::interactive::sub::command::InteractiveSubCommand;

/// Format the CLI prompt with the version of the binary
pub(crate) async fn format_sub_prompt(version: &str, name: &str) -> anyhow::Result<String> {
    let prompt_stack: Vec<String> = vec![
        format!("themelio-client").cyan().bold().to_string(),
        format!("(v{})", version).magenta().to_string(),
        format!("➜ ").cyan().bold().to_string(),
        format!("(v{})", name).cyan().to_string(),
        format!("➜ ").cyan().bold().to_string(),
    ];
    Ok(format!("{}", prompt_stack.join(" ")))
}

/// Get user input and parse it into a wallet_shell command
pub(crate) async fn read_line(prompt: &str) -> anyhow::Result<InteractiveSubCommand> {
    let input = common_read_line(prompt.to_string()).await?;

    let open_wallet_cmd = InteractiveSubCommand::try_from(input.to_string())?;
    Ok(open_wallet_cmd)
}