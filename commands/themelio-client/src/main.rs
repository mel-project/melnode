mod storage;
mod wallet;

use structopt::StructOpt;
use crate::wallet::dispatcher::WalletDispatcher;
use std::str::FromStr;

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

    #[structopt(subcommand)]
    sub: Option<WalletSubCommand>,
}

#[derive(StructOpt, Debug)]
enum WalletSubCommand {
    Create {
        wallet_name: String
    },
    Faucet {
        amount: String,
        unit: String
    },
    SendCoins {
        address: String,
        amount: String,
        unit: String
    },
    AddCoins {
        coin_id: String
    },
    Deposit,
    Withdraw,
    Swap,
    Balance,
    Show,
    Exit
}
//
// use std::fmt;
//
// impl fmt::Display for OpenWalletSubCommand {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         write!(f, "{:?}", self)
//     }
// }
// use std::fmt::{Display, Formatter};
//
// impl Display for OpenWalletSubCommand {
//     fn fmt(&self, f: &mut Formatter<'_>) -> Result {
//        Ok("hi")
//     }
// }
// impl FromStr for OpenWalletSubCommand {
//     // type Error = ScanError;
//     //
//     // /// Uses serde scan internally to parse a whitespace delimited string into a command
//     // fn try_from(value: String) -> Result<Self, Self::Error> {
//     //     let cmd: Result<OpenWalletSubCommand, _> = serde_scan::from_str(&value);
//     //     cmd
//     // }
//
//     type Err = ();
//
//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         let x = OpenWalletSubCommand::Exit;
//         Ok(x)
//     }
// }

// eprintln!("\nAvailable commands are: ");
// eprintln!(">> faucet <amount> <unit>");
// eprintln!(">> send-coins <address> <amount> <unit>");
// eprintln!(">> add-coins <coin-id>");
// // eprintln!(">> deposit args");
// // eprintln!(">> swap args");
// // eprintln!(">> withdraw args");
// eprintln!(">> balance");
// eprintln!(">> help");
// eprintln!(">> exit");
// eprintln!(">> ");

/// Run client with command line options
fn main() {
    let version = env!("CARGO_PKG_VERSION");
    let opts: ClientOpts = ClientOpts::from_args();
    smolscale::block_on(async move {
        let dispatcher = WalletDispatcher::new(&opts.host, &opts.database, version);
        dispatcher.run().await
    }).unwrap();
}
