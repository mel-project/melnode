use std::{convert::TryInto, io, sync::Arc};

use erased_serde::{Serialize, Serializer};
use executor::CommandExecutor;
use nodeprot::ValClient;
use storage::SledMap;
use structopt::StructOpt;
use tmelcrypt::HashVal;

use crate::config::{DEFAULT_TRUST_HEADER_HASH, DEFAULT_TRUST_HEIGHT};
use crate::context::ExecutionContext;
use crate::opts::{ClientOpts, ClientSubOpts, WalletUtilsCommand};
use crate::shell::runner::WalletShellRunner;
use crate::wallet::data::WalletData;

mod config;
mod context;
mod executor;
mod opts;
mod shell;
mod wallet;

/// Parse options from input arguments and asynchronously dispatch associated command.
fn main() {
    smolscale::block_on(async move {
        let opts: ClientOpts = ClientOpts::from_args();
        dispatch(opts).await.expect("Failed to execute command");
    });
}

/// Open an interactive wallet shell or execute a wallet utils command using input options.
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
            let ser: Box<dyn Serialize> = match cmd {
                WalletUtilsCommand::CreateWallet { wallet_name } => {
                    let info = executor.create_wallet(&wallet_name).await?;
                    Box::new(info) as Box<dyn Serialize>
                }
                WalletUtilsCommand::Faucet {
                    wallet_name,
                    secret,
                    amount,
                    unit,
                } => {
                    let info = executor
                        .faucet(&wallet_name, &secret, &amount, &unit)
                        .await?;
                    Box::new(info) as Box<dyn Serialize>
                }
                WalletUtilsCommand::SendCoins {
                    wallet_name,
                    secret,
                    address,
                    amount,
                    unit,
                } => {
                    let info = executor
                        .send_coins(&wallet_name, &secret, &address, &amount, &unit)
                        .await?;
                    Box::new(info) as Box<dyn Serialize>
                }
                WalletUtilsCommand::AddCoins {
                    wallet_name,
                    secret,
                    coin_id,
                } => {
                    let info = executor.add_coins(&wallet_name, &secret, &coin_id).await?;
                    Box::new(info) as Box<dyn Serialize>
                }
                WalletUtilsCommand::ShowBalance {
                    wallet_name,
                    secret,
                } => {
                    let info = executor.show_balance(&wallet_name, &secret).await?;
                    Box::new(info) as Box<dyn Serialize>
                }
                WalletUtilsCommand::ShowWallets => {
                    let info = executor.show_wallets().await?;
                    Box::new(info) as Box<dyn Serialize>
                }
                WalletUtilsCommand::DepositCoins {
                    wallet_name,
                    secret,
                    cov_hash_a,
                    amount_a,
                    cov_hash_b,
                    amount_b,
                } => {
                    let info = executor
                        .deposit(
                            &wallet_name,
                            &secret,
                            &cov_hash_a,
                            &amount_a,
                            &cov_hash_b,
                            &amount_b,
                        )
                        .await?;
                    Box::new(info) as Box<dyn Serialize>
                }
                WalletUtilsCommand::WithdrawCoins {
                    wallet_name,
                    secret,
                    cov_hash_a,
                    amount_a,
                    cov_hash_b,
                    amount_b,
                } => {
                    let info = executor
                        .withdraw(
                            &wallet_name,
                            &secret,
                            &cov_hash_a,
                            &amount_a,
                            &cov_hash_b,
                            &amount_b,
                        )
                        .await?;
                    Box::new(info) as Box<dyn Serialize>
                }
                WalletUtilsCommand::SwapCoins {
                    wallet_name,
                    secret,
                    cov_hash,
                    amount,
                } => {
                    let info = executor
                        .swap(&wallet_name, &secret, &cov_hash, &amount)
                        .await?;
                    Box::new(info) as Box<dyn Serialize>
                }
            };

            // Show results serialized as JSON
            let json = &mut serde_json::Serializer::new(io::stdout());
            ser.erased_serialize(&mut Serializer::erase(json))?;
        }
    }

    Ok(())
}
