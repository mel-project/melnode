// use std::{convert::TryInto, net::SocketAddr, env};

// use themelio_stf::{melvm, CoinID};
// use colored::Colorize;
// use std::io::prelude::*;
// use structopt::StructOpt;
// use tabwriter::TabWriter;

// use crate::config::{VERSION, SQL_FILE_NAME};
// use crate::services::{ActiveWallet, AvailableWallets, WalletData};

// #[derive(Debug, StructOpt)]
// pub struct AnetClientConfig {
//     /// Address for bootstrapping into the network
//     #[structopt(long, default_value = "94.237.109.44:11814")]
//     bootstrap: SocketAddr,
// }

// /// Runs the alphanet client
// pub async fn run_anet_client(cfg: AnetClientConfig) {
//     // wallets
//     let mut storage_path = env::var("CARGO_MANIFEST_DIR").unwrap();
//     storage_path.push_str(SQL_FILE_NAME);
//     let available_wallets = AvailableWallets::new(&storage_path);

//     let mut prompt_stack: Vec<String> = vec![format!("v{}", VERSION).green().to_string()];
//     loop {
//         let prompt = format!("[anet client {}]% ", prompt_stack.join(" "));
//         let res = try_run_prompt(&mut prompt_stack, &prompt, &available_wallets, &cfg, &storage_path).await;
//         match res {
//             Ok(exit) => {
//                 if exit {
//                     break;
//                 }
//             }
//             Err(err) => { eprintln!(">> {}: {}", "ERROR".red().bold(), err); }
//         }
//     }
// }

// async fn try_run_prompt(
//     prompt_stack: &mut Vec<String>,
//     prompt: &str,
//     available_wallets: &AvailableWallets,
//     cfg: &AnetClientConfig,
//     storage_path: &String
// ) -> anyhow::Result<bool> {
//     let input = read_line(prompt.to_string()).await.unwrap();
//     let mut tw = TabWriter::new(vec![]);

//     match input.split(' ').collect::<Vec<_>>().as_slice() {
//         &["wallet_shell-new", wallet_name] => {
//             if available_wallets.get(wallet_name).is_some() {
//                 eprintln!(">> {}: data already exists", "ERROR".red().bold());
//                 return Ok(false);
//             }
//             let (sk, _pk, wallet_data) = WalletData::generate();
//             let wallet_shell = available_wallets.insert(wallet_name, &wallet_data);
//             assert!(!wallet_shell, "Internal error: DB state inconsistent");
//             writeln!(tw, ">> New data:\t{}", wallet_name.bold()).unwrap();
//             writeln!(
//                 tw,
//                 ">> Address:\t{}",
//                 wallet_data.my_script.hash().to_addr().yellow()
//             )
//             .unwrap();
//             writeln!(tw, ">> Secret:\t{}", hex::encode(sk.0).dimmed()).unwrap();
//             tw.flush().unwrap();
//         }
//         &["wallet_shell-unlock", wallet_name, wallet_secret] => {
//             if let Some(wallet_shell) = available_wallets.get(&wallet_name) {
//                 let wallet_secret = hex::decode(wallet_secret)?;
//                 let wallet_secret = tmelcrypt::Ed25519SK(wallet_secret.as_slice().try_into()?);
//                 if melvm::Covenant::std_ed25519_pk(wallet_secret.to_public()) != wallet_shell.my_script {
//                     return Err(anyhow::anyhow!(
//                         "unlocking failed, make sure you have the right secret!"
//                     ));
//                 }
//                 prompt_stack.push(format!("({})", wallet_name).yellow().to_string());
//                 let prompt = format!("[anet client {}]% ", prompt_stack.join(" "));
//                 loop {
//                     let mut active_wallet = ActiveWallet::new(
//                         wallet_secret,
//                         wallet_shell.clone(),
//                         cfg.bootstrap,
//                         &storage_path,
//                     );
//                     let res = run_active_wallet(wallet_name, &mut active_wallet, &prompt).await;
//                     match res {
//                         Ok(_) => {
//                             break;
//                         }
//                         Err(err) => {
//                             eprintln!("Error encountered when running sub wallet_shell {}", err);
//                             continue;
//                         }
//                     }
//                 }
//                 prompt_stack.pop();
//             }
//         }
//         &["wallet_shell-list"] => {
//             let wallets = available_wallets.get_all();
//             writeln!(tw, ">> [NAME]\t[ADDRESS]")?;
//             for (name, wallet_shell) in wallets.iter() {
//                 writeln!(tw, ">> {}\t{}", name, wallet_shell.my_script.hash().to_addr())?;
//             }
//         }
//         &["exit"] => {
//             return Ok(true);
//         }
//         _other => {
//             eprintln!("\nAvailable commands are: ");
//             eprintln!(">> wallet_shell-new <wallet_shell-name>");
//             eprintln!(">> wallet_shell-unlock <wallet_shell-name> <secret>");
//             eprintln!(">> wallet_shell-list");
//             eprintln!(">> exit");
//             return Ok(false);
//         }
//     }
//     tw.flush()?;
//     eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
//     Ok(false)
// }

// async fn read_line(prompt: String) -> anyhow::Result<String> {
//     smol::unblock(move || {
//         let mut rl = rustyline::Editor::<()>::new();
//         Ok(rl.readline(&prompt)?)
//     })
//     .await
// }

// /// Handle command line inputs for sub wallet_shell mode
// async fn run_active_wallet(
//     wallet_name: &str,
//     active_wallet: &mut ActiveWallet,
//     prompt: &str,
// ) -> anyhow::Result<()> {
//     loop {
//         let input = read_line(prompt.to_string()).await.unwrap();
//         match input.split(' ').collect::<Vec<_>>().as_slice() {
//             ["faucet", number, unit] => {
//                 let coin = active_wallet.send_faucet_tx(number, unit).await?;
//                 eprintln!(
//                     ">> Faucet transaction for {} mels broadcast!",
//                     number.to_string().bold()
//                 );
//                 eprintln!(">> Waiting for confirmation...");
//                 // loop until we get coin data height and proof from last header
//                 loop {
//                     let (coin_data_height, _hdr) = active_wallet.get_coin_data(coin).await?;
//                     if let Some(cd_height) = coin_data_height {
//                         eprintln!(
//                             ">>> Coin is confirmed at current height {}",
//                             cd_height.height
//                         );

//                         eprintln!(
//                             ">> CID = {}",
//                             hex::encode(stdcode::serialize(&coin).unwrap()).bold()
//                         );
//                         break;
//                     }
//                 }
//             }
//             ["coin-add", coin_id] => {
//                 let (coin_data_height, coin_id, _full_proof) =
//                     active_wallet.get_coin_data_by_id(coin_id).await?;
//                 match coin_data_height {
//                     None => {
//                         eprintln!("Coin not found");
//                         continue;
//                     }
//                     Some(coin_data_height) => {
//                         eprintln!(
//                             ">> Coin found at height {}! Added {} {} to data",
//                             coin_data_height.height,
//                             coin_data_height.coin_data.value,
//                             {
//                                 let val = coin_data_height.coin_data.denom.as_slice();
//                                 format!("X-{}", hex::encode(val))
//                             }
//                         );
//                         active_wallet.add_coin(&coin_id, &coin_data_height).await?;
//                         eprintln!("Added coin to wallet_shell");
//                     }
//                 }
//             }
//             ["send-tx", dest_addr, amount, unit] => {
//                 let tx = active_wallet.create_tx(dest_addr, amount, unit).await?;
//                 let fee_prompt = format!("Do you wish to send a tx with a fee of {} (y/n): ", tx.fee);
//                 let fee_input = read_line(fee_prompt.to_string()).await.unwrap();
//                 if !fee_input.contains('y') {
//                     continue;
//                 }

//                 let tx = active_wallet.send_tx(tx).await?;
//                 eprintln!(">> Sent tx.  Waiting to verify.");
//                 loop {
//                     let (coin_data_height, _proof) = active_wallet.verify_tx(tx.clone()).await?;
//                     if let Some(out) = coin_data_height {
//                         let their_coin = CoinID {
//                             txhash: tx.hash_nosigs(),
//                             index: 0,
//                         };
//                         let first_change = CoinID {
//                             txhash: tx.hash_nosigs(),
//                             index: 1,
//                         };
//                         eprintln!(">> Confirmed at height {}!", out.height);
//                         eprintln!(
//                             ">> CID (Sent) = {}",
//                             hex::encode(stdcode::serialize(&their_coin).unwrap()).bold()
//                         );
//                         eprintln!(
//                             ">> CID (Change) = {}",
//                             hex::encode(stdcode::serialize(&first_change).unwrap()).bold()
//                         );
//                         break;
//                     }
//                 }
//             }
//             ["coins",] => {
//                 let unspent_coins = active_wallet.get_unspent_coins().await?;
//                 eprintln!(">> **** COINS ****");

//                 let mut tw = TabWriter::new(vec![]);
//                 writeln!(&mut tw, ">> [CoinID]\t[Height]\t[Amount]\t[CoinType]").unwrap();

//                 for (coin_id, coin_data) in unspent_coins.iter() {
//                     let coin_id = hex::encode(stdcode::serialize(coin_id).unwrap());
//                     write!(&mut tw,
//                            ">> {}\t{}\t{}\t{}",
//                            coin_id,
//                            coin_data.height.to_string(),
//                            coin_data.coin_data.value.to_string(),
//                            {
//                                "μTML"
//                            },
//                     ).unwrap();
//                 }
//                 tw.flush().unwrap();
//                 println!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap())

//             }
//             ["spent-coins",] => {
//                 let spent_coins = active_wallet.get_spent_coins().await?;
//                 eprintln!(">> **** COINS ****");

//                 let mut tw = TabWriter::new(vec![]);
//                 writeln!(&mut tw, ">> [CoinID]\t[Height]\t[Amount]\t[CoinType]").unwrap();

//                 for (coin_id, coin_data) in spent_coins.iter() {
//                     let coin_id = hex::encode(stdcode::serialize(coin_id).unwrap());
//                     write!(&mut tw,
//                            ">> {}\t{}\t{}\t{}",
//                            coin_id,
//                            coin_data.height.to_string(),
//                            coin_data.coin_data.value.to_string(),
//                            {
//                                "μTML"
//                            },
//                     ).unwrap();
//                 }
//                 tw.flush().unwrap();
//                 println!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap())

//             }
//             ["balance",] => {
//                 let balance = active_wallet.get_balance().await?;
//                 eprintln!(">> **** BALANCE ****");
//                 eprintln!(">> {}", balance);
//             }
//             ["exit",] => {
//                 return Ok(());
//             }
//             _ => {
//                 eprintln!("\nAvailable commands are: ");
//                 eprintln!(">> faucet <amount> <unit>");
//                 eprintln!(">> coin-add <coin-id>");
//                 eprintln!(">> coins");
//                 eprintln!(">> spent-coins");
//                 eprintln!(">> balance");
//                 eprintln!(">> send-tx <address> <amount> <unit>");
//                 eprintln!(">> exit");
//                 continue;
//             }
//         }
//         active_wallet.save(wallet_name).await?;
//     }
// }
