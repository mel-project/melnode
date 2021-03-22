mod wallet;

use structopt::StructOpt;
use crate::wallet::storage::WalletStorage;
use crate::wallet::handler::{PromptHandler, WalletCommand};

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

fn main() {
    let opts: ClientOpts = ClientOpts::from_args();
    smolscale::block_on(run_client(opts));
}

async fn run_client(opts: ClientOpts) -> anyhow::Result<()> {
    // Create prompt handler from client, storage and package version
    let client = nodeprot::ValClient::new(opts.network, opts.host);
    let storage = WalletStorage::new(sled::open(&opts.database).unwrap());
    let prompt = PromptHandler::new(client, storage, env!("CARGO_PKG_VERSION"));

    // Handle prompt input and output until user selects exit command
    loop {
        let res = prompt.handle().await;
        if res.is_ok() && res.unwrap() == WalletCommand::Exit {
            Ok(())
        }
    }
}
