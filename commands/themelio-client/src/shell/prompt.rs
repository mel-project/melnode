use std::convert::TryFrom;
use colored::Colorize;
use crate::shell::command::{ShellCommand, SubShellCommand};
use crate::wallet::error::WalletError;

pub(crate) fn format_prompt(version: &str) -> String {
    let prompt_stack: Vec<String> = vec![
        "themelio-client".to_string().cyan().bold().to_string(),
        format!("(v{})", version).magenta().to_string(),
        "➜ ".to_string().cyan().bold().to_string(),
    ];
    prompt_stack.join(" ")
}

pub(crate) fn format_named_prompt(version: &str, name: &str) -> String {
    let prompt_stack: Vec<String> = vec![
        "themelio-client".to_string().cyan().bold().to_string(),
        format!("(v{})", version).magenta().to_string(),
        "➜ ".to_string().cyan().bold().to_string(),
        format!("({})", name).cyan().to_string(),
        "➜ ".to_string().cyan().bold().to_string(),
    ];
    prompt_stack.join(" ")
}

pub(crate) async fn read_shell_command(prompt: &str) -> anyhow::Result<ShellCommand> {
    let input = common_read_line(prompt.to_string()).await?;
    let wallet_cmd = ShellCommand::try_from(input.clone());
    if wallet_cmd.is_err() {
        anyhow::bail!(WalletError::InvalidInputArgs(input))
    }
    Ok(wallet_cmd?)
}


pub(crate) async fn read_sub_shell_command(prompt: &str) -> anyhow::Result<SubShellCommand> {
    let input = common_read_line(prompt.to_string()).await?;
    let open_wallet_cmd = SubShellCommand::try_from(input)?;
    Ok(open_wallet_cmd)
}

/// Helper method that read_line method in trait can call to handle raw user input.
pub(crate) async fn common_read_line(prompt: String) -> anyhow::Result<String> {
    smol::unblock(move || {
        let mut rl = rustyline::Editor::<()>::new();
        Ok(rl.readline(&prompt)?)
    }).await
}
