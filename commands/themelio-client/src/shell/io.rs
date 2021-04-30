use anyhow::Error;

use crate::shell::command::{ShellCommand, SubShellCommand};
use crate::wallet::error::WalletError;
use colored::Colorize;
use std::convert::TryFrom;

/// Create a prompt for shell mode
pub(crate) fn format_prompt(version: &str) -> String {
    let prompt_stack: Vec<String> = vec![
        "themelio-client".to_string().cyan().bold().to_string(),
        format!("(v{})", version).magenta().to_string(),
        "➜ ".to_string().cyan().bold().to_string(),
    ];
    prompt_stack.join(" ")
}

/// Create a named prompt for sub shell mode to show wallet name.
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

/// Read input to prompt and parse it into a shell command
pub(crate) async fn read_shell_command(prompt: &str) -> anyhow::Result<ShellCommand> {
    let input = common_read_line(prompt.to_string()).await?;
    let wallet_cmd = ShellCommand::try_from(input.clone());
    if wallet_cmd.is_err() {
        anyhow::bail!(WalletError::InvalidInputArgs(input))
    }
    Ok(wallet_cmd?)
}

/// Read input to prompt and parse it into a sub shell command
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
    })
    .await
}

/// Output the error when dispatching command.
pub(crate) fn print_command_error(err: &Error, cmd: &ShellCommand) {
    eprintln!("ERROR: {} with wallet_shell command {:?}", err, cmd);
}

/// Output the error when reading user input.
pub(crate) fn print_readline_error(_err: &Error) {
    eprintln!("ERROR: can't parse input command");
}

/// Show exit message.
pub(crate) fn print_exit_message() {
    eprintln!("\nExiting Themelio Client wallet_shell");
}

/// Output the error when dispatching command
pub(crate) fn print_dispatch_error(err: &Error, sub_cmd: &SubShellCommand) {
    eprintln!("ERROR: {} when dispatching {:?}", err, sub_cmd);
}

/// Show available input commands for the shell
pub(crate) fn print_shell_help() {
    eprintln!("\nAvailable commands are: ");
    eprintln!(">> create-wallet <wallet-name>");
    eprintln!(">> open-wallet <wallet-name> <secret>");
    eprintln!(">> show");
    eprintln!(">> help");
    eprintln!(">> exit");
    eprintln!(">> ");
}

/// Show available input commands for the sub shell
pub(crate) fn print_subshell_help() {
    eprintln!("\nAvailable commands are: ");
    eprintln!(">> faucet <amount> <unit>");
    eprintln!(">> send-coins <address> <amount> <unit>");
    eprintln!(">> add-coins <coin-id>");
    // eprintln!(">> deposit args");
    // eprintln!(">> swap args");
    // eprintln!(">> withdraw args");
    eprintln!(">> balance");
    eprintln!(">> help");
    eprintln!(">> exit");
    eprintln!(">> ");
}

/// Show exit message
pub(crate) fn print_shell_exit() {
    eprintln!("\nExiting Themelio Client wallet shell");
}

/// Show exit message
pub(crate) fn print_subshell_exit() {
    eprintln!("\nExiting Themelio Client active wallet");
}
