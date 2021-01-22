use std::{convert::TryInto, net::SocketAddr};

use blkstructs::{melscript, CoinID};
use colored::Colorize;
use std::io::prelude::*;
use structopt::StructOpt;
use tabwriter::TabWriter;

use crate::config::VERSION;
use crate::services::{ActiveWallet, AvailableWallets, WalletData};

#[derive(Debug, StructOpt)]
pub struct AnetClientConfig {
    /// Address for bootstrapping into the network
    #[structopt(long, default_value = "94.237.109.44:11814")]
    bootstrap: SocketAddr,

    /// Path to db storage
    #[structopt(long, default_value = "./sql.db")]
    storage_path: String,
}

/// Runs the alphanet client
pub async fn run_anet_client(cfg: AnetClientConfig) {
    // wallets
    let available_wallets = AvailableWallets::new(&cfg.storage_path);

    let mut prompt_stack: Vec<String> = vec![format!("v{}", VERSION).green().to_string()];
    loop {
        let prompt = format!("[anet client {}]% ", prompt_stack.join(" "));
        let res = try_run_prompt(&mut prompt_stack, &prompt, &available_wallets, &cfg).await;
        if let Err(err) = res {
            eprintln!(">> {}: {}", "ERROR".red().bold(), err);
        }
    }
}

async fn try_run_prompt(
    prompt_stack: &mut Vec<String>,
    prompt: &str,
    available_wallets: &AvailableWallets,
    cfg: &AnetClientConfig,
) -> anyhow::Result<()> {
    let input = read_line(prompt.to_string()).await.unwrap();
    let mut tw = TabWriter::new(vec![]);

    match input.split(' ').collect::<Vec<_>>().as_slice() {
        &["wallet-new", wallet_name] => {
            if available_wallets.get(wallet_name).is_some() {
                eprintln!(">> {}: data already exists", "ERROR".red().bold());
                return Ok(());
            }
            let (sk, _pk, wallet_data) = WalletData::generate();
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
                let wallet_secret = tmelcrypt::Ed25519SK(wallet_secret.as_slice().try_into()?);
                if melscript::Script::std_ed25519_pk(wallet_secret.to_public()) != wallet.my_script
                {
                    return Err(anyhow::anyhow!(
                        "unlocking failed, make sure you have the right secret!"
                    ));
                }
                prompt_stack.push(format!("({})", wallet_name).yellow().to_string());
                let prompt = format!("[anet client {}]% ", prompt_stack.join(" "));
                loop {
                    let mut active_wallet = ActiveWallet::new(
                        wallet_secret,
                        wallet.clone(),
                        cfg.bootstrap,
                        &cfg.storage_path,
                    );
                    let res = run_active_wallet(wallet_name, &mut active_wallet, &prompt).await;
                    match res {
                        Ok(_) => {
                            break;
                        }
                        Err(err) => {
                            eprintln!("Error encountered when running active wallet {}", err);
                            continue;
                        }
                    }
                }
                prompt_stack.pop();
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
            return Ok(());
        }
    }
    tw.flush()?;
    eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
    Ok(())
}

async fn read_line(prompt: String) -> anyhow::Result<String> {
    smol::unblock(move || {
        let mut rl = rustyline::Editor::<()>::new();
        Ok(rl.readline(&prompt)?)
    })
    .await
}

/// Handle command line inputs for active wallet mode
async fn run_active_wallet(
    wallet_name: &str,
    active_wallet: &mut ActiveWallet,
    prompt: &str,
) -> anyhow::Result<()> {
    loop {
        let input = read_line(prompt.to_string()).await.unwrap();
        match input.split(' ').collect::<Vec<_>>().as_slice() {
            ["faucet", number, unit] => {
                let coin = active_wallet.send_faucet_tx(number, unit).await?;
                eprintln!(
                    ">> Faucet transaction for {} mels broadcast!",
                    number.to_string().bold()
                );
                eprintln!(">> Waiting for confirmation...");
                // loop until we get coin data height and proof from last header
                loop {
                    let (coin_data_height, _hdr) = active_wallet.get_coin_data(coin).await?;
                    if let Some(cd_height) = coin_data_height {
                        eprintln!(
                            ">>> Coin is confirmed at current height {}",
                            cd_height.height
                        );

                        eprintln!(
                            ">> CID = {}",
                            hex::encode(bincode::serialize(&coin).unwrap()).bold()
                        );
                        break;
                    }
                }
            }
            ["coin-add", coin_id] => {
                let (coin_data_height, coin_id, _full_proof) =
                    active_wallet.get_coin_data_by_id(coin_id).await?;
                match coin_data_height {
                    None => {
                        eprintln!("Coin not found");
                        continue;
                    }
                    Some(coin_data_height) => {
                        eprintln!(
                            ">> Coin found at height {}! Added {} {} to data",
                            coin_data_height.height,
                            coin_data_height.coin_data.value,
                            match coin_data_height.coin_data.cointype.as_slice() {
                                // COINTYPE_TMEL => "μmel".to_string(),
                                val => format!("X-{}", hex::encode(val)),
                            }
                        );
                        active_wallet.add_coin(&coin_id, &coin_data_height).await?;
                        eprintln!("Added coin to wallet");
                    }
                }
            }
            ["tx-send", dest_addr, amount, unit] => {
                let tx = active_wallet.create_tx(dest_addr, amount, unit).await?;
                eprintln!(">> Tx fee is {}", tx.fee);

                let tx = active_wallet.send_tx(tx).await?;
                eprintln!(">> Sent tx.  Waiting to verify.");
                loop {
                    let (coin_data_height, _proof) = active_wallet.verify_tx(tx.clone()).await?;
                    if let Some(out) = coin_data_height {
                        let their_coin = CoinID {
                            txhash: tx.hash_nosigs(),
                            index: 0,
                        };
                        eprintln!(">> Confirmed at height {}!", out.height);
                        eprintln!(
                            ">> CID = {}",
                            hex::encode(bincode::serialize(&their_coin).unwrap()).bold()
                        );
                        break;
                    }
                }
            }
            ["balances",] => {
                let unspent_coins = active_wallet.get_balances().await?;
                eprintln!(">> **** COINS ****");
                eprintln!(">> [CoinID]\t[Height]\t[Amount]\t[CoinType]");
                for (coin_id, coin_data) in unspent_coins.iter() {
                    let coin_id = hex::encode(bincode::serialize(coin_id).unwrap());
                    eprintln!(
                        ">> {}\t{}\t{}\t{}",
                        coin_id,
                        coin_data.height.to_string(),
                        coin_data.coin_data.value.to_string(),
                        match coin_data.coin_data.cointype.as_slice() {
                            _COINTYPE_TMEL => "μTML",
                            // _ => "(other)",
                        },
                    );
                }
            }
            ["exit",] => {
                return Ok(());
            }
            _ => {
                eprintln!("Invalid command for active wallet");
                continue;
            }
        }
        active_wallet.save(wallet_name).await?;
    }
}
