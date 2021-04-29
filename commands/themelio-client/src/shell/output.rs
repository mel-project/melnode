use anyhow::Error;

use crate::shell::command::ShellCommand;

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

use crate::shell::sub::command::SubShellCommand;

/// Send coins to a recipient.
pub(crate) async fn sent_coins() {}

/// Add coins into wallet storage.
pub(crate) async fn added_coins() {}

/// Transfer coins from faucet to your wallet.
async fn faucet_tx(amt: &str, denom: &str) -> anyhow::Result<()> {
    Ok(())
}

/// Output the error when dispatching command
pub(crate) async fn dispatch_error(err: &Error, sub_cmd: &SubShellCommand) -> anyhow::Result<()> {
    eprintln!("ERROR: {} when dispatching {:?}", err, sub_cmd);
    Ok(())
}

/// Output the error when reading user input.
pub(crate) async fn readline_error(_err: &Error) -> anyhow::Result<()> {
    eprintln!("ERROR: can't parse input command");
    Ok(())
}

/// Show available input commands
pub(crate) async fn help() -> anyhow::Result<()> {
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
    Ok(())
}

/// Show exit message
pub(crate) async fn exit() -> anyhow::Result<()> {
    eprintln!("\nExiting Themelio Client sub-wallet_shell");
    Ok(())
}
