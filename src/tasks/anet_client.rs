use std::{collections::HashMap, convert::TryInto, net::SocketAddr};

use blkstructs::{
    melscript, CoinData, CoinID, Transaction, TxKind, COINTYPE_TMEL, MICRO_CONVERTER,
};
use colored::Colorize;
use std::io::prelude::*;
use structopt::StructOpt;
use tabwriter::TabWriter;

use crate::config::VERSION;
use crate::services::{AvailableWallets, Client, WalletData};
use std::path::Path;

#[derive(Debug, StructOpt)]
pub struct AnetClientConfig {
    /// Address for bootstrapping into the network
    #[structopt(long, default_value = "35.225.14.194:18888")]
    bootstrap: SocketAddr,

    /// Path to db storage
    #[structopt(long, default_value = "./sql.db")]
    storage_path: String,
}

/// Runs the alphanet client
pub async fn run_anet_client(cfg: AnetClientConfig) {
    let mut prompt_stack: Vec<String> = vec![format!("v{}", VERSION).green().to_string()];

    // wallets
    let available_wallets = AvailableWallets::new(&cfg.storage_path);

    // let mut current_wallet: Option<(String, tmelcrypt::Ed25519SK)> = None;
    let mut client = Client::new(cfg.bootstrap);

    loop {
        let prompt = format!("[anet client {}]% ", prompt_stack.join(" "));
        let res: anyhow::Result<()> = try {
            let input = read_line(prompt).await.unwrap();
            let mut tw = TabWriter::new(vec![]);
            // data mode
            if false {
                //let Some((wallet_name, wallet_sk)) = &mut current_wallet {
                // let wallet = wallets.get_mut(wallet_name).unwrap();
                match input.split(' ').collect::<Vec<_>>().as_slice() {
                    ["faucet", number, unit] => {
                        // let (coin_data, height) = active_wallet.fuacet(number, unit);
                        // display_faucet(coin_data, height);
                    }
                    ["coin-add", coin_id] => {
                        // let (coin_id, height) = active_wallet.coin_add(coin_id);
                        // display_coin_add(coin_id, height);
                    }
                    ["tx-send", dest_addr, amount, unit] => {
                        // let height = active_wallet.tx_send(dest_addr, amount, unit);
                        // display_tx_send(height);
                    }
                    ["balances"] => {
                        // let balances = active_wallet.get_balances();
                        // display_balances(prompt_stack, balances);
                    }
                    ["exit"] => {
                        prompt_stack.pop();
                        // current_wallet = None;
                    }
                    _ => Err(anyhow::anyhow!("no such command"))?,
                }
            } else {
                match input.split(' ').collect::<Vec<_>>().as_slice() {
                    &["data-new", wallet_name] => {
                        if let Some(existing_wallet) = available_wallets.get(wallet_name) {
                            eprintln!(">> {}: data already exists", "ERROR".red().bold());
                            continue;
                        }
                        let (sk, pk, wallet_data) = WalletData::generate();
                        let wallet = available_wallets.insert(wallet_name, &wallet_data);
                        assert!(!wallet, "DB state inconsistent");
                        writeln!(tw, ">> New data:\t{}", wallet_name.bold()).unwrap();
                        writeln!(
                            tw,
                            ">> Address:\t{}",
                            wallet_data.my_script.hash().to_addr().yellow()
                        )
                        .unwrap();
                        writeln!(tw, ">> Secret:\t{}", hex::encode(sk.0).dimmed()).unwrap();
                        tw.flush().unwrap();
                    }
                    &["data-unlock", wallet_name, wallet_secret] => {
                        // available_wallets.unlock(wallet_name, wallet_secret);
                        // display_available_wallets_unlock()
                    }
                    &["data-list"] => {
                        // available_wallets.list();
                    }
                    other => {
                        eprintln!("no such command: {:?}", other);
                        continue;
                    }
                }
            }
            tw.flush()?;
            eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
        };
        if let Err(err) = res {
            eprintln!(">> {}: {}", "ERROR".red().bold(), err);
        }
    }
}

async fn read_line(prompt: String) -> anyhow::Result<String> {
    smol::unblock(move || {
        let mut rl = rustyline::Editor::<()>::new();
        Ok(rl.readline(&prompt)?)
    })
    .await
}
