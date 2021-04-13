use crate::common::read_line;
use crate::shell::sub::command::SubShellCommand;
use anyhow::Error;
use colored::Colorize;
use std::convert::TryFrom;

pub struct SubShellInput {}

impl SubShellInput {
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

    /// Get user input and parse it into a shell command
    pub(crate) async fn command(prompt: &str) -> anyhow::Result<SubShellCommand> {
        let input = read_line(prompt.to_string()).await?;

        let open_wallet_cmd = SubShellCommand::try_from(input.to_string())?;
        Ok(open_wallet_cmd)
    }
}

pub struct SubShellOutput {}

impl SubShellOutput {

    /// Send coins to a recipient.
    pub(crate) async fn sent_coins() {}

    /// Add coins into wallet storage.
    pub(crate) async fn added_coins() {}

    /// Transfer coins from faucet to your wallet.
    async fn faucet_tx(&self, amt: &str, denom: &str) -> anyhow::Result<()> {
        // let shell = Wallet::new(&self.host, &self.database);
        //
        // let wallet_data = shell.sub(&self.name, &self.secret).await?;
        //
        // let coin = shell.faucet(&wallet_data, self.amt, self.denom).await?;
        //
        // prompter::output_faucet_tx(wallet_data, coin).await?;
        //
        // self.confirm_faucet_tx(coin).await?;
        //
        // prompter::faucet_tx_confirmed().await?;

        Ok(())
    }
//
//     async fn confirm_faucet(&self, _coin_id: CoinID) -> anyhow::Result<()> {
//         // loop {
//         //
//         //     prompter::faucet_tx_confirming().await?;
//         // }
//         //                 eprintln!(
// //                     ">> Faucet transaction for {} mels broadcast!",
// //                     number.to_string().bold()
// //                 );
// //                 eprintln!(">> Waiting for confirmation...");
// //                 // loop until we get coin data height and proof from last header
// //                 loop {
// //                     let (coin_data_height, _hdr) = active_wallet.get_coin_data(coin).await?;
// //                     if let Some(cd_height) = coin_data_height {
// //                         eprintln!(
// //                             ">>> Coin is confirmed at current height {}",
// //                             cd_height.height
// //                         );
//
// //                         eprintln!(
// //                             ">> CID = {}",
// //                             hex::encode(stdcode::serialize(&coin).unwrap()).bold()
// //                         );
// //                         break;
// //                     }
//         Ok(())
//     }

    /// Output the error when dispatching command
    pub(crate) async fn error(err: &Error, sub_shell_cmd: &SubShellCommand) -> anyhow::Result<()> {
        eprintln!("ERROR: {} when dispatching {:?}", err, sub_shell_cmd);
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
        eprintln!("\nExiting Themelio Client sub-shell");
        Ok(())
    }
}
