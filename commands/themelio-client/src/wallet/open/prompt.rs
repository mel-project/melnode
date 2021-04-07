use crate::wallet::common::read_line;
use crate::wallet::open::command::OpenWalletCommand;
use anyhow::Error;

/// Format the CLI prompt with the version of the binary
pub(crate) async fn format_prompt(version: &str, name: &str) -> anyhow::Result<String> {
    let prompt_stack: Vec<String> = vec![
        format!("themelio-client").cyan().bold().to_string(),
        format!("(v{})", version).magenta().to_string(),
        format!("➜ ").cyan().bold().to_string(),
        format!("(v{})", name).cyan().to_string(),
        format!("➜ ").cyan().bold().to_string(),
    ];
    Ok(format!("{}", prompt_stack.join(" ")))
}

/// Get user input and parse it into a wallet command
pub(crate) async fn input_command(prompt: &str) -> anyhow::Result<OpenWalletCommand> {
    let input = read_line(prompt.to_string()).await?;

    let open_wallet_cmd = OpenWalletCommand::try_from(input.to_string())?;
    Ok(open_wallet_cmd)
}

// output_cmd_error

/// Output the error when dispatching command
pub(crate) async fn output_cmd_error(err: &Error, wallet_cmd: &OpenWalletCommand) -> anyhow::Result<()> {
    eprintln!("ERROR: {} when dispatching {:?}", err, wallet_cmd);
    Ok(())
}

/// Show available input commands
pub(crate) async fn output_help() -> anyhow::Result<()> {
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
pub(crate) async fn output_exit() -> anyhow::Result<()> {
    eprintln!("\nExiting Themelio Client Open Wallet Mode");
    Ok(())
}