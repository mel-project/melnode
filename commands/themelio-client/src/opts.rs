use structopt::StructOpt;

#[derive(Debug, Clone, StructOpt)]
#[structopt(name = "Themelio Client CLI")]
/// A command line interface for clients to interact with Themelio.
///
/// The wallet-shell mode is suitable for human interaction to manage wallets and transact with the network.
///
/// The wallet_utils mode is more suitable automation and
/// executes one-line commands with custom formatter formats like JSON.
pub struct ClientOpts {
    /// IP Address with port used to establish a connection to host
    #[structopt(long, short, default_value = "127.0.0.1:8000")]
    pub host: smol::net::SocketAddr,

    /// File path to database for client wallet_shell storage
    #[structopt(long, short, parse(from_os_str), default_value = "/tmp/testclient")]
    pub database: std::path::PathBuf,

    /// Time to sleep in seconds while polling for transaction confirmation data from node.
    #[structopt(long, short, default_value = "5")]
    pub sleep_sec: u64,

    /// Contains all the sub-command options for a client
    #[structopt(subcommand)]
    pub sub_opts: ClientSubOpts,
}

#[derive(StructOpt, Clone, Debug)]
/// Contains the wallet-shell and wallet_utils mode.
pub enum ClientSubOpts {
    WalletShell,
    WalletUtils(WalletUtilsCommand),
}

#[derive(StructOpt, Clone, Debug)]
/// Represents the command options to run a specific command.
/// TODO: add descriptions
pub enum WalletUtilsCommand {
    CreateWallet {
        wallet_name: String,
    },
    Faucet {
        wallet_name: String,
        secret: String,
        amount: String,
        unit: String,
    },
    SendCoins {
        wallet_name: String,
        secret: String,
        address: String,
        amount: String,
        unit: String,
    },
    AddCoins {
        wallet_name: String,
        secret: String,
        coin_id: String,
    },
    DepositCoins {
        wallet_name: String,
        secret: String,
        cov_hash_a: String,
        amount_a: String,
        cov_hash_b: String,
        amount_b: String,
    },
    WithdrawCoins {
        wallet_name: String,
        secret: String,
        cov_hash_a: String,
        amount_a: String,
        cov_hash_b: String,
        amount_b: String,
    },
    SwapCoins {
        wallet_name: String,
        secret: String,
        cov_hash: String,
        amount: String,
    },
    ShowBalance {
        wallet_name: String,
        secret: String,
    },
    ShowWallets,
}
