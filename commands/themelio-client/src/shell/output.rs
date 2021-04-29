use anyhow::Error;

use crate::shell::command::{ShellCommand, SubShellCommand};

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
pub(crate)  fn print_dispatch_error(err: &Error, sub_cmd: &SubShellCommand)  {
    eprintln!("ERROR: {} when dispatching {:?}", err, sub_cmd);
}

/// Show available input commands
pub(crate) fn print_help() {
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
    eprintln!("\nExiting Themelio Client Wallet Shell");
}

/// Show exit message
pub(crate) fn print_subshell_exit() {
    eprintln!("\nExiting Themelio Client Active Wallet");
}