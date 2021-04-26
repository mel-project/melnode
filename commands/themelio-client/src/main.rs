use crate::opts::{ClientOpts, ClientSubOpts, OutputFormat, WalletUtilsCommand};
use crate::shell::runner::WalletShellRunner;
use crate::utils::context::ExecutionContext;
use nodeprot::ValClient;
use std::{convert::TryInto, sync::Arc};
use structopt::StructOpt;
use tmelcrypt::HashVal;
use utils::executor::CommandExecutor;
use crate::config::{DEFAULT_TRUST_HEIGHT, DEFAULT_TRUST_HEADER_HASH};
use storage::SledMap;
use crate::wallet::data::WalletData;

mod opts;
mod shell;
mod utils;
mod wallet;
mod config;

/// Parse options from input arguments and asynchronously dispatch them.
fn main() {
    smolscale::block_on(async move {
        let opts: ClientOpts = ClientOpts::from_args();
        dispatch(opts).await.expect("Failed to execute command");
    });
}

/// Either start the wallet shell runner or invoke a utils command using an executor.
async fn dispatch(opts: ClientOpts) -> anyhow::Result<()> {
    let version = env!("CARGO_PKG_VERSION").to_string();
    let network = blkstructs::NetID::Testnet;
    let host = opts.host;
    let sled_map = SledMap::<String, WalletData>::new(
        sled::open(&opts.database)?.open_tree(b"wallet_storage")?,
    );
    let database = Arc::new(sled_map);
    let sleep_sec = opts.sleep_sec;
    let client = ValClient::new(network, host);
    client.trust(
        DEFAULT_TRUST_HEIGHT,
        HashVal(
            hex::decode(DEFAULT_TRUST_HEADER_HASH)?
                .try_into()
                .unwrap(),
        ),
    );

    match opts.sub_opts {
        ClientSubOpts::WalletShell => {
            let formatter = Arc::new(OutputFormat::default());
            let context = ExecutionContext {
                version,
                network,
                host,
                database,
                sleep_sec,
                client,
                formatter,
            };
            let runner = WalletShellRunner::new(context);
            runner.run().await
        }
        ClientSubOpts::WalletUtils(util_opts) => {
            let formatter = Arc::new(util_opts.output_format.make());
            let context = ExecutionContext {
                version,
                network,
                host,
                database,
                client,
                sleep_sec,
                formatter,
            };
            let executor = CommandExecutor::new(context);
            match util_opts.cmd {
                WalletUtilsCommand::CreateWallet { wallet_name } => {
                    executor.create_wallet(&wallet_name).await
                }
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
