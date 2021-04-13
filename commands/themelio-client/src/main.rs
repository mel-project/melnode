pub mod lib;

use lib::{executor, options::{Opts, SubOpts}, shell::executor::ShellExecutor};
use structopt::StructOpt;

/// Run a single dispatch given command line options.
fn main() {
    smolscale::block_on(async move {
        let version = env!("CARGO_PKG_VERSION");
        let opts: Opts = Opts::from_args();
        run_command(opts, version).await
    });
}

/// Run the command given the options input by the user
pub async fn run_command(opts: Opts, version: &str) -> anyhow::Result<()> {
    let adapter = executor::ClientExecutor::new(opts.host, opts.database, false);
    match opts.sub_opts {
        SubOpts::CreateWallet { wallet_name } => {
            adapter.create_wallet(&wallet_name).await?
        }
        SubOpts::Faucet { wallet_name, secret, amount, unit } => {
            adapter.faucet(&wallet_name, &secret, &amount, &unit).await?
        }
        SubOpts::SendCoins { wallet_name, secret, address, amount, unit } => {
            adapter.send_coins(&wallet_name, &secret, &address, &amount, &unit).await?
        }
        SubOpts::AddCoins { wallet_name, secret, coin_id } => {
            adapter.add_coins(&wallet_name, &secret, &coin_id).await?
        }
        SubOpts::ShowBalance { wallet_name, secret } => {
            adapter.show_balance(&wallet_name, &secret,).await?
        }
        SubOpts::ShowWallets => {
            adapter.show_wallets().await?
        }
        SubOpts::Shell => {
            let executor = ShellExecutor::new(&adapter.host, &adapter.database, version);
            executor.run().await?
        }
    }
    Ok(())
}