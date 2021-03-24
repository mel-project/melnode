mod storage;
mod wallet;

use crate::wallet::command::{WalletCommand, WalletCommandHandler};
use storage::ClientStorage;
use structopt::StructOpt;

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
    smolscale::block_on(run_client_prompt(opts));
}

/// Handle a prompt until exit command
async fn run_client_prompt(opts: ClientOpts) -> anyhow::Result<()> {
    let handler = WalletCommandHandler::new(
        opts.host,
        opts.database,
        env!("CARGO_PKG_VERSION").to_string(),
    );

    loop {
        let res_cmd = handler.handle().await;
        if res_cmd.is_ok() && res_cmd.unwrap() == WalletCommand::Exit {
            return Ok(());
        }
    }
}
