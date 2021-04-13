pub mod lib;

use lib::{executor, options::{Opts, SubOpts}, shell::runner::ShellRunner};
use structopt::StructOpt;

/// Run a single dispatch given command line options.
fn main() {
    smolscale::block_on(async move {
        let version = env!("CARGO_PKG_VERSION");
        let opts: Opts = Opts::from_args();
        run_command(opts, version).await
    });
}

/// Run the command given the command line options input from the user.
pub async fn run_command(opts: Opts, version: &str) -> anyhow::Result<()> {
    let executor = executor::CommandExecutor::new(opts.host, opts.database, false);
    match opts.sub_opts {
        SubOpts::CreateWallet { wallet_name } => { executor.create_wallet(&wallet_name).await? }
        SubOpts::Faucet { wallet_name, secret, amount, unit } => {
            executor.faucet(&wallet_name, &secret, &amount, &unit).await?
        }
        SubOpts::SendCoins { wallet_name, secret, address, amount, unit } => {
            executor.send_coins(&wallet_name, &secret, &address, &amount, &unit).await?
        }
        SubOpts::AddCoins { wallet_name, secret, coin_id } => {
            executor.add_coins(&wallet_name, &secret, &coin_id).await?
        }
        SubOpts::ShowBalance { wallet_name, secret } => {
            executor.show_balance(&wallet_name, &secret).await?
        }
        SubOpts::ShowWallets => {
            executor.show_wallets().await?
        }
        SubOpts::Shell => {
            let executor = ShellRunner::new(&adapter.host, &adapter.database, version);
            executor.run().await?
        }
    }
    Ok(())
}
