mod wallet;

use structopt::StructOpt;
use std::path::PathBuf;

use wallet::handler::handle_prompt;
use crate::wallet::storage::WalletStorage;

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
    database: PathBuf,
}

fn main() {
    let opts: ClientOpts = ClientOpts::from_args();
    smolscale::block_on(run_client(opts.host, opts.database))
}

async fn run_client(host: smol::net::SocketAddr, database: PathBuf) {
    let prompt = WalletPrompt::new();
    let db: sled::Db = sled::Db::new(database);
    let mut storage = WalletStorage::new(db);

    loop {
        let prompt_result = handle_prompt(&prompt, &storage).await?;
        // handle res err handling if any here
    }
}
