use crate::common::context::ExecutionContext;
use crate::opts::{ClientOpts, ClientSubOpts, WalletUtilsCommand, OutputFormat};
use structopt::StructOpt;
use crate::wallet_shell::runner::WalletShellRunner;
use common::executor::CommandExecutor;
use std::sync::Arc;

mod common;
mod wallet;
mod wallet_shell;
mod opts;

/// Parse options from input arguments and asynchronously dispatch them.
fn main() {
    smolscale::block_on(async move {
        let opts: ClientOpts = ClientOpts::from_args();
        dispatch(opts).await.expect("Failed to execute command");
    });
}

/// Either start the wallet shell runner or invoke a utils command using an executor.
async fn dispatch(opts: ClientOpts) -> anyhow::Result<()>{
    let version = env!("CARGO_PKG_VERSION").to_string();
    let network = blkstructs::NetID::Testnet;
    let host = opts.host;
    let database = opts.database;
    let sleep_sec = opts.sleep_sec;
    let fee = opts.fee;

    match opts.sub_opts {
        ClientSubOpts::WalletShell => {
            let formatter = Arc::new(OutputFormat::default());
            let context = ExecutionContext {
                version, network, host, database, sleep_sec, fee, formatter
            };
            let runner = WalletShellRunner::new(context);
            runner.run().await
        }
        ClientSubOpts::WalletUtils(util_opts) => {
            let formatter = Arc::new(util_opts.output_format.make());
            let context = ExecutionContext {
                version, network, host, database, sleep_sec, fee, formatter
            };
            let executor = CommandExecutor::new(context);
            match util_opts.cmd {
                WalletUtilsCommand::CreateWallet { wallet_name } => {
                    executor.create_wallet(&wallet_name).await
                },
                WalletUtilsCommand::Faucet {
                    wallet_name,
                    secret,
                    amount,
                    unit,
                } => executor.faucet(&wallet_name, &secret, &amount, &unit).await,
                WalletUtilsCommand::SendCoins {
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
                WalletUtilsCommand::AddCoins {
                    wallet_name,
                    secret,
                    coin_id,
                } => executor.add_coins(&wallet_name, &secret, &coin_id).await,
                WalletUtilsCommand::ShowBalance {
                    wallet_name,
                    secret,
                } => executor.show_balance(&wallet_name, &secret).await,
                WalletUtilsCommand::ShowWallets => executor.show_wallets().await,
            }
        }
    }
}
