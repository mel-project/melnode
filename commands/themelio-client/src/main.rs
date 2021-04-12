use structopt::StructOpt;
use crate::command::ClientSubcommand;
use crate::dispatcher::ClientDispatcher;

pub mod shell;
pub mod common;
pub mod error;
pub mod wallet;
pub mod command;
pub mod dispatcher;

#[derive(Debug, StructOpt)]
#[structopt(name = "Themelio Client CLI")]
/// A command line application to interact with a Themelio node
///
/// This binary supports all features for a Themelio client.
/// End users can create, delete, import, and export wallets.
/// Interacting with the blockchain is done by opening a shell
/// and creating and sending various transactions.
pub struct ClientOpts {
    /// IP Address with port used to establish a connection to host
    #[structopt(long, default_value="127.0.0.1:8000")]
    host: smol::net::SocketAddr,

    /// File path to database for client shell storage
    #[structopt(long, short, parse(from_os_str), default_value="/tmp/testclient")]
    database: std::path::PathBuf,

    #[structopt(subcommand)]
    subcommand: ClientSubcommand,
}

/// Run client with command line options
fn main() {
    let version = env!("CARGO_PKG_VERSION");
    let opts: ClientOpts = ClientOpts::from_args();
    let dispatcher = ClientDispatcher::new(&opts.host, &opts.database, version);
    smolscale::block_on(async move {
        let res = dispatcher.dispatch(opts.subcommand).await;
    }).unwrap();
}
