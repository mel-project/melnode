use crate::utils::formatter::formatter::OutputFormatter;
use crate::utils::formatter::json::JsonOutputFormatter;
use crate::utils::formatter::plain::PlainOutputFormatter;
use serde::{Deserialize, Serialize};
use serde_scan::ScanError;
use std::str::FromStr;
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

    /// Time to sleep in seconds while polling for transaction confirmation data from node.
    #[structopt(long, short, default_value = "1051000000")]
    pub fee: u128,

    /// Contains all the sub-command options for a client
    #[structopt(subcommand)]
    pub sub_opts: ClientSubOpts,
}

#[derive(StructOpt, Clone, Debug)]
/// Contains the wallet-shell and wallet_utils mode.
pub enum ClientSubOpts {
    WalletShell,
    WalletUtils(WalletUtilsOpts),
}

/// Represents how to format formatter.
#[derive(StructOpt, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    Plain,
    Json,
}

impl OutputFormat {
    pub fn default() -> Box<dyn OutputFormatter + Sync + Send + 'static> {
        Box::new(PlainOutputFormatter {})
    }

    pub fn make(&self) -> Box<dyn OutputFormatter + Sync + Send + 'static> {
        return match self {
            OutputFormat::Plain => Box::new(PlainOutputFormatter {}),
            OutputFormat::Json => Box::new(JsonOutputFormatter {}),
        };
    }
}

/// Uses serde scan to parse a string into an formatter format enum.
impl FromStr for OutputFormat {
    type Err = ScanError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let cmd: Result<OutputFormat, _> = serde_scan::from_str(value);
        cmd
    }
}

#[derive(StructOpt, Clone, Debug)]
/// Allows end user to select an formatter format on a specific util command.
pub struct WalletUtilsOpts {
    /// Select how to format the formatter of a utility command.
    #[structopt(long, short)]
    pub output_format: OutputFormat,

    /// Automation-centric commands to executed with the exception of the wallet_shell 'wallet_shell' command.
    #[structopt(subcommand)]
    pub cmd: WalletUtilsCommand,
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
    // TODO: determine how to handle fee input for wallet_shell and non-wallet_shell case
    // ie... do we add in optional field for handling fee input?
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
}
