use structopt::StructOpt;

#[derive(Debug, Clone, StructOpt)]
#[structopt(name = "Themelio Client CLI")]
/// A command line application to interact with a Themelio node
///
/// This binary supports all features for a Themelio client.
/// End users can create, delete, import, and export wallets.
/// Interacting with the blockchain is done by opening a shell
/// and creating and sending various transactions.
pub struct Opts {
    /// IP Address with port used to establish a connection to host
    #[structopt(long, default_value="127.0.0.1:8000")]
    pub host: smol::net::SocketAddr,

    /// File path to database for client shell storage
    #[structopt(long, short, parse(from_os_str), default_value="/tmp/testclient")]
    pub database: std::path::PathBuf,

    #[structopt(subcommand)]
    pub sub_opts: SubOpts,
}

#[derive(StructOpt, Clone, Debug)]
/// Represents the sub options to run a specific command.
/// If Shell is specified it will enter into an interactive shell,
/// otherwise it will execute a single command and exit.
pub enum SubOpts {
    CreateWallet {
        wallet_name: String
    },
    Faucet {
        wallet_name: String,
        secret: String,
        amount: String,
        unit: String
    },
    // TODO: determine how to handle fee input for interactive and non-interactive case
    // ie... do we add in optional field for handling fee input?
    SendCoins {
        wallet_name: String,
        secret: String,
        address: String,
        amount: String,
        unit: String
    },
    AddCoins {
        wallet_name: String,
        secret: String,
        coin_id: String
    },
    // TODO: Add in correct fields for deposit, withdraw and swap
    // DepositCoins {
    //     wallet_name: String,
    //     secret: String,
    //     covhash_a: String,
    //     amount_a: String,
    //     covhash_b: String,
    //     amount_b: String,
    // },
    // WithdrawCoins {
    //     wallet_name: String,
    //     secret: String,
    //     covhash_a: String,
    //     amount_a: String,
    //     covhash_b: String,
    //     amount_b: String,
    // },
    // SwapCoins {
    //     wallet_name: String,
    //     secret: String,
    //     covhash: String,
    //     amount: String,
    // },
    ShowBalance {
        wallet_name: String,
        secret: String,
    },
    ShowWallets,
    Shell
}
