use std::{convert::TryInto, io, sync::Arc};

use erased_serde::Serializer;
use nodeprot::ValClient;
use storage::SledMap;
use structopt::StructOpt;
use tmelcrypt::HashVal;
use utils::executor::CommandExecutor;

use crate::config::{DEFAULT_TRUST_HEADER_HASH, DEFAULT_TRUST_HEIGHT};
use crate::opts::{ClientOpts, ClientSubOpts, WalletUtilsCommand};
use crate::shell::runner::WalletShellRunner;
use crate::utils::context::ExecutionContext;
use crate::wallet::data::WalletData;

mod config;
mod opts;
mod shell;
mod utils;
mod wallet;

/// Parse options from input arguments and asynchronously dispatch them.
fn main() {
    smolscale::block_on(async move {
        let opts: ClientOpts = ClientOpts::from_args();
        dispatch(opts).await.expect("Failed to execute command");
    });
}

/// Either start the wallet shell or execute a wallet utils command.
async fn dispatch(opts: ClientOpts) -> anyhow::Result<()> {
    // Initialize execution context
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
        HashVal(hex::decode(DEFAULT_TRUST_HEADER_HASH)?.try_into().unwrap()),
    );
    let context = ExecutionContext {
        version,
        network,
        host,
        database,
        sleep_sec,
        client,
    };

    // Run in either wallet shell or utils mode.
    match opts.sub_opts {
        ClientSubOpts::WalletShell => {
            let runner = WalletShellRunner::new(context);
            runner.run().await?
        }
        ClientSubOpts::WalletUtils(cmd) => {
            let executor = CommandExecutor::new(context);

            // Execute command and get serializable results
            let ser = match cmd {
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
                } => {
                    todo!("impl");
                }
                WalletUtilsCommand::ShowBalance {
                    wallet_name,
                    secret,
                } => {
                    todo!("impl");
                }
                WalletUtilsCommand::ShowWallets => {
                    todo!("impl");
                }
                WalletUtilsCommand::DepositCoins {
                    wallet_name,
                    secret,
                    cov_hash_a,
                    amount_a,
                    cov_hash_b,
                    amount_b,
                } => {
                    todo!("impl");
                }
                WalletUtilsCommand::WithdrawCoins {
                    wallet_name,
                    secret,
                    cov_hash_a,
                    amount_a,
                    cov_hash_b,
                    amount_b,
                } => {
                    todo!("impl");
                }
                WalletUtilsCommand::SwapCoins {
                    wallet_name,
                    secret,
                    cov_hash,
                    amount,
                } => {
                    todo!("impl");
                }
            }?;

            // Show results serialized as JSON
            let json = &mut serde_json::Serializer::new(io::stdout());
            ser.erased_serialize(&mut Serializer::erase(json))?;
        }
    }

    Ok(())
}
