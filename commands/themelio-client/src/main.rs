use structopt::StructOpt;

use crate::wallet::handler::{PromptHandler, Command};
use crate::wallet::storage::ClientStorage;
use anyhow::Error;

mod wallet;

#[derive(Debug, StructOpt)]
#[structopt(name = "Themelio Client CLI")]
/// A command line application to interact with Themelio
///
/// This binary supports all features for a Themelio client.
/// End users can create, delete, import, and export wallets.
/// Interacting with the blockchain is done by opening a specific wallet
/// and creating and sending various supported transactions.
pub struct ClientOpts {
    /// IP Address with port used to establish a connection to host
    #[structopt(long)]
    host: smol::net::SocketAddr,

    // File path to database for client wallet storage
    #[structopt(long, short, parse(from_os_str), default_value="/tmp/testclient")]
    database: std::path::PathBuf,

    // Specify whether we are connecting to mainnet or testnet
    #[structopt(long)]
    network: blkstructs::NetID
}

/// Run client with command line options
fn main() {
    let opts: ClientOpts = ClientOpts::from_args();
    smolscale::block_on(run_client_prompt(opts));
}

/// Handle a prompt until exit command
async fn run_client_prompt(opts: ClientOpts) -> anyhow::Result<()> {
    let client = nodeprot::ValClient::new(opts.network, opts.host);
    let storage = ClientStorage::new(sled::open(&opts.database).unwrap());
    let prompt = PromptHandler::new(client, storage, env!("CARGO_PKG_VERSION"));

    loop {
        let res_cmd = prompt.handle().await;
        if res_cmd.is_ok() && res_cmd.unwrap() == Command::Exit {
            Ok(())
        }
    }
}
