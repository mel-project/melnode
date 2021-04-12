use structopt::StructOpt;

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
    pub(crate) host: smol::net::SocketAddr,

    /// File path to database for client shell storage
    #[structopt(long, short, parse(from_os_str), default_value="/tmp/testclient")]
    pub(crate) database: std::path::PathBuf,

    #[structopt(subcommand)]
    pub(crate) subcommand: ClientSubOpts,
}

#[derive(StructOpt, Debug)]
/// Represents the sub options to run a specific command.
/// If Shell is specified it will enter into an interactive shell,
/// otherwise it will execute a single command and exit.
pub(crate) enum ClientSubOpts {
    CreateWallet {
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
    // TODO: Add in correct fields for deposit, withdraw and swap
    // DepositCoins {
    //     covhash_a: String,
    //     amount_a: String,
    //     covhash_b: String,
    //     amount_b: String,
    // },
    // WithdrawCoins {
    //     covhash_a: String,
    //     amount_a: String,
    //     covhash_b: String,
    //     amount_b: String,
    // },
    // SwapCoins {
    //     covhash: String,
    //     amount: String,
    // },
    ShowBalance,
    ShowWallets,
    Shell,
    Exit
}
