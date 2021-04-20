use crate::common::context::ExecutionContext;
use crate::opts::{ClientOpts, ClientSubOpts, WalletUtilsCommand};
use structopt::StructOpt;
use crate::wallet_shell::runner::WalletShellRunner;
use crate::wallet_utils::executor::WalletUtilsExecutor;

mod common;
mod wallet;
mod wallet_shell;
mod wallet_utils;
mod opts;

/// Parse options from input arguments and asynchronously dispatch them.
fn main() {
    smolscale::block_on(async move {
        let opts: ClientOpts = ClientOpts::from_args();
        dispatch(opts).await.expect("Failed to execute command");
    });
}

/// Convert options into an execution context and then dispatch a command.
async fn dispatch(opts: ClientOpts) -> anyhow::Result<()>{
    let context = ExecutionContext {
        version: env!("CARGO_PKG_VERSION").to_string(),
        network: blkstructs::NetID::Testnet,
        host: opts.host,
        database: opts.database,
        sleep_sec: opts.sleep_sec,
        fee: opts.fee
    };
    match opts.sub_opts {
        ClientSubOpts::WalletShell => {
            let runner = WalletShellRunner::new(context);
            runner.run().await
        }
        ClientSubOpts::WalletUtils(util_opts) => {
            let formatter = util_opts.output_format.make();
            let executor = WalletUtilsExecutor::new(context, formatter);
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
