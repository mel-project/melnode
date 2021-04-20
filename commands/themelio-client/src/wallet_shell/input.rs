use std::convert::TryFrom;

use colored::Colorize;

use crate::common::input::read_line as common_read_line;
use crate::interactive::command::InteractiveCommand;

/// Format the wallet_shell prompt with the version of the binary.
pub(crate) async fn format_prompt(version: &str) -> anyhow::Result<String> {
    let prompt_stack: Vec<String> = vec![
        format!("themelio-client").cyan().bold().to_string(),
        format!("(v{})", version).magenta().to_string(),
        format!("➜ ").cyan().bold().to_string(),
    ];
    Ok(format!("{}", prompt_stack.join(" ")))
}

/// Get user input and parse it into a wallet_shell command.
pub(crate) async fn read_line(prompt: &str) -> anyhow::Result<InteractiveCommand> {
    let input = read_line_inner(prompt.to_string()).await?;
    let wallet_cmd = InteractiveCommand::try_from(input.to_string())?;
    Ok(wallet_cmd)
}

/// Handle raw user input using a prompt.
async fn read_line_inner(prompt: String) -> anyhow::Result<String> {
    smol::unblock(move || {
        let mut rl = rustyline::Editor::<()>::new();
        Ok(rl.readline(&prompt)?)
    })
        .await
}
