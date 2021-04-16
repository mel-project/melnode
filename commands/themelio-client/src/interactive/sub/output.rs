use anyhow::Error;

use crate::interactive::sub::command::InteractiveSubCommand;

/// Send coins to a recipient.
pub(crate) async fn sent_coins() {}

/// Add coins into wallet storage.
pub(crate) async fn added_coins() {}

/// Transfer coins from faucet to your wallet.
async fn faucet_tx(amt: &str, denom: &str) -> anyhow::Result<()> {
    Ok(())
}

/// Output the error when dispatching command
pub(crate) async fn subshell_error(
    err: &Error,
    sub_shell_cmd: &InteractiveSubCommand,
) -> anyhow::Result<()> {
    eprintln!("ERROR: {} when dispatching {:?}", err, sub_shell_cmd);
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
    eprintln!("\nExiting Themelio Client sub-interactive");
    Ok(())
}
