mod storage;
mod wallet;

use structopt::StructOpt;
use crate::wallet::dispatcher::WalletDispatcher;

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

    /// File path to database for client wallet storage
    #[structopt(long, short, parse(from_os_str), default_value = "/tmp/testclient")]
    database: std::path::PathBuf,
}

/// Run client with command line options
fn main() {
    let version = env!("CARGO_PKG_VERSION");
    let opts: ClientOpts = ClientOpts::from_args();
    let dispatcher = WalletDispatcher::new(&opts.host, &opts.database, version);
    smolscale::block_on(dispatcher.run()).unwrap();
}
