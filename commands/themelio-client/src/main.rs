pub mod common;
pub mod executor;
pub mod interactive;
pub mod noninteractive;
pub mod wallet;

use crate::executor::NonInteractiveCommandExecutor;
use structopt::StructOpt;

#[derive(Debug, Clone, StructOpt)]
#[structopt(name = "Themelio Client CLI")]
/// A command line application to interact with Themelio.
///
/// An interactive interactive can be used as a CLI wallet allowing a user
/// to store wallet state data locally as well as send and query data from the network.
///
/// Other non-interactive command options are suitable for automation
/// and can execute a single command to completion given all the arguments.
pub struct Opts {
    /// IP Address with port used to establish a connection to host
    #[structopt(long, default_value = "127.0.0.1:8000")]
    pub host: smol::net::SocketAddr,

    /// File path to database for client interactive storage
    #[structopt(long, short, parse(from_os_str), default_value = "/tmp/testclient")]
    pub database: std::path::PathBuf,

    /// Automation-centric commands to executed with the exception of the interactive 'interactive' command.
    #[structopt(subcommand)]
    pub cmd_opts: CommandOpts,
}

#[derive(StructOpt, Clone, Debug)]
/// Represents the command options to run a specific command.
/// If 'interactive' is specified it will enter into an interactive interactive,
/// otherwise it will execute a single command and exit.
///
/// TODO: add descriptions
pub enum CommandOpts {
    CreateWallet {
        wallet_name: String,
    },
    Faucet {
        wallet_name: String,
        secret: String,
        amount: String,
        unit: String,
    },
    // TODO: determine how to handle fee input for interactive and non-interactive case
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
    Shell,
}

/// Parse options from input arguments and asynchronously dispatch them.
fn main() {
    smolscale::block_on(async move {
        let opts: Opts = Opts::from_args();
        dispatch(opts).await.expect("Failed to execute command");
    });
}

/// Convert options into an execution context and then dispatch a command.
async fn dispatch(opts: Opts) -> anyhow::Result<()> {
    let context = common::context::ExecutionContext {
        version: env!("CARGO_PKG_VERSION").to_string(),
        network: blkstructs::NetID::Testnet,
        host: opts.host,
        database: opts.database,
        default_sleep_sec: 5, // TODO: maybe make this come in from opts?
        default_fee: 2050000000,
    };
    let executor = NonInteractiveCommandExecutor::new(context);
    match opts.cmd_opts {
        CommandOpts::CreateWallet { wallet_name } => executor.create_wallet(&wallet_name).await,
        CommandOpts::Faucet {
            wallet_name,
            secret,
            amount,
            unit,
        } => executor.faucet(&wallet_name, &secret, &amount, &unit).await,
        CommandOpts::SendCoins {
            wallet_name,
            secret,
            address,
            amount,
            unit,
        } => {
            executor
                .send_coins(&wallet_name, &secret, &address, &amount, &unit)
                .await
        }
        CommandOpts::AddCoins {
            wallet_name,
            secret,
            coin_id,
        } => executor.add_coins(&wallet_name, &secret, &coin_id).await,
        CommandOpts::ShowBalance {
            wallet_name,
            secret,
        } => executor.show_balance(&wallet_name, &secret).await,
        CommandOpts::ShowWallets => executor.show_wallets().await,
        CommandOpts::Shell => executor.shell().await,
    }
}
