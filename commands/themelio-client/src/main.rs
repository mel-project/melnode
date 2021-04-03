mod storage;
mod wallet;
mod lib;

use structopt::StructOpt;
use crate::wallet::command::WalletCommandResult;
use crate::wallet::dispatcher::WalletCommandDispatcher;
use crate::wallet::prompt::WalletPrompt;

#[derive(Debug, StructOpt)]
#[structopt(name = "Themelio Client CLI")]
/// A command line application to interact with a Themelio node
///
/// This binary supports all features for a Themelio client.
/// End users can create, delete, import, and export wallets.
/// Interacting with the blockchain is done by opening a wallet
/// and creating and sending various transactions.
pub struct ClientOpts {
    /// IP Address with port used to establish a connection to host
    #[structopt(long)]
    host: smol::net::SocketAddr,

    // File path to database for client wallet storage
    #[structopt(long, short, parse(from_os_str), default_value = "/tmp/testclient")]
    database: std::path::PathBuf,
}

/// Run client with command line options
fn main() {
    let opts: ClientOpts = ClientOpts::from_args();
    let dispatcher = WalletCommandDispatcher::new(&opts.host, &opts.database);
    smolscale::block_on(run_client_prompt(&dispatcher, env!("CARGO_PKG_VERSION"))).unwrap();
}

/// Execute a command and process the command result until a user exits
async fn run_client_prompt(dispatcher: &WalletCommandDispatcher, version: &str) -> anyhow::Result<()> {
    let prompt = WalletPrompt::new(version);
    loop {
        // Get command from user input
        let (wallet_cmd, open_wallet_cmd) = prompt.input().await?;

        // Dispatch the command
        let dispatcher_result = dispatcher.dispatch(&wallet_cmd, &open_wallet_cmd).await;

        // Handle the dispatcher result
        match dispatcher_result {
            Ok(cmd_res) => {
                // Output command results
                prompt.output_result(&cmd_res).await?;

                // Check for exit command
                if cmd_res == WalletCommandResult::Exit {
                    return Ok(());
                }
            }
            Err(err) => {
                // Output error description
                prompt.output_error(&err, &wallet_cmd);
            }
        }
    }
}
