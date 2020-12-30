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
                    &["wallet-new", wallet_name] => {
                        if let Some(existing_wallet) = available_wallets.get(wallet_name) {
                            eprintln!(">> {}: data already exists", "ERROR".red().bold());
                            continue;
                        }
                        let (sk, pk, wallet_data) = WalletData::generate();
                        let wallet = available_wallets.insert(wallet_name, &wallet_data);
                        assert!(!wallet, "Internal error: DB state inconsistent");
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
                    &["wallet-unlock", wallet_name, wallet_secret] => {
                        if let Some(wallet) = available_wallets.get(&wallet_name) {
                            let wallet_secret = hex::decode(wallet_secret)?;
                            let wallet_secret =
                                tmelcrypt::Ed25519SK(wallet_secret.as_slice().try_into()?);
                            if melscript::Script::std_ed25519_pk(wallet_secret.to_public())
                                != wallet.my_script
                            {
                                Err(anyhow::anyhow!(
                                    "unlocking failed, make sure you have the right secret!"
                                ))?;
                            }
                            // TODO: how to handle current wallet?
                            // current_wallet = Some((wallet_name.to_string(), wallet_secret));
                            prompt_stack.push(format!("({})", wallet_name).yellow().to_string());
                        }
                    }
                    &["wallet-list"] => {
                        let wallets = available_wallets.get_all();
                        writeln!(tw, ">> [NAME]\t[ADDRESS]")?;
                        for (name, wallet) in wallets.iter() {
                            writeln!(tw, ">> {}\t{}", name, wallet.my_script.hash().to_addr())?;
                        }
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
