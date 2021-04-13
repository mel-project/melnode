pub mod options;
pub mod shell;
pub mod common;
pub mod error;
pub mod wallet;
pub mod executor;

use options::{Opts, SubOpts};
use executor::CommandExecutor;

use structopt::StructOpt;

/// Run a single dispatch given command line options.
fn main() {
    smolscale::block_on(async move {
        let version = env!("CARGO_PKG_VERSION");
        let opts: Opts = Opts::from_args();
        let _ = run_command(opts, version).await;
    });
}

/// Run the command given the command line options input from the user.
pub async fn run_command(opts: Opts, version: &str) -> anyhow::Result<()> {
    let ce = CommandExecutor::new(opts.host, opts.database, version);
    match opts.sub_opts {
        SubOpts::CreateWallet { wallet_name } => ce.create_wallet(&wallet_name).await?,
        SubOpts::Faucet { wallet_name, secret, amount, unit } => ce.faucet(&wallet_name, &secret, &amount, &unit).await?,
        SubOpts::SendCoins { wallet_name, secret, address, amount, unit } => ce.send_coins(&wallet_name, &secret, &address, &amount, &unit).await?,
        SubOpts::AddCoins { wallet_name, secret, coin_id } => ce.add_coins(&wallet_name, &secret, &coin_id).await?,
        SubOpts::ShowBalance { wallet_name, secret } => ce.show_balance(&wallet_name, &secret).await?,
        SubOpts::ShowWallets => ce.show_wallets().await?,
        SubOpts::Shell => ce.shell().await?,
    }
    Ok(())
}
