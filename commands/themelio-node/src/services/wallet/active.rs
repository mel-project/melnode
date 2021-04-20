// use crate::services::WalletData;
// use anyhow::Context;
// use blkstructs::{
//     CoinData, CoinDataHeight, CoinID, Header, Transaction, TxKind, DENOM_TMEL, MICRO_CONVERTER,
// };
// use smol::net::SocketAddr;
// use tmelcrypt::Ed25519SK;
//
// use autosmt::FullProof;
// use rusqlite::Connection;
// use std::collections::HashMap;
// use std::path::Path;
//
// pub struct ActiveWallet {
//     client: NetClient,
//     sk: Ed25519SK,
//     wallet_shell: WalletData,
//     conn: Connection,
// }
//
// impl ActiveWallet {
//     pub fn new(sk: Ed25519SK, wallet_shell: WalletData, remote: SocketAddr, path: &str) -> Self {
//         let path = Path::new(path);
//         let conn = Connection::sub(path).expect("SQLite connection failure");
//         wallet_shell::init(&conn).expect("Failed to load wallet_shell");
//         ActiveWallet {
//             sk,
//             wallet_shell,
//             client: NetClient::new(remote),
//             conn,
//         }
//     }
//
//     pub async fn send_faucet_tx(&mut self, number: &str, unit: &str) -> anyhow::Result<CoinID> {
//         // validate input
//         let number: u128 = number.parse()?;
//         assert_eq!(unit, "TML");
//         // create faucet transaction and broadcast it
//         let fee = 2000000; // TODO: better fee estimation for faucet tx
//         let txn = Transaction {
//             kind: TxKind::Faucet,
//             inputs: vec![],
//             outputs: vec![CoinData {
//                 denom: DENOM_TMEL.to_owned(),
//                 covhash: self.wallet_shell.my_script.hash(),
//                 value: number * MICRO_CONVERTER,
//             }],
//             fee,
//             scripts: vec![],
//             sigs: vec![],
//             data: vec![],
//         };
//
//         let coin = CoinID {
//             txhash: txn.hash_nosigs(),
//             index: 0,
//         };
//
//         self.client
//             .broadcast_tx(txn)
//             .await
//             .expect("Error in broadcast_tx");
//
//         Ok(coin)
//     }
//
//     pub async fn get_coin_data(
//         &mut self,
//         coin: CoinID,
//     ) -> anyhow::Result<(Option<CoinDataHeight>, Header)> {
//         let (hdr, _) = self.client.last_header().await?;
//         let (cdh, _proof) = self.client.get_coin(hdr, coin).await?;
//         Ok((cdh, hdr))
//     }
//
//     pub async fn get_coin_data_by_id(
//         &mut self,
//         coin_id: &str,
//     ) -> anyhow::Result<(Option<CoinDataHeight>, CoinID, autosmt::FullProof)> {
//         eprintln!(">> Syncing state...");
//         let header = self.client.last_header().await?.0;
//         eprintln!(">> Retrieving coin at height {}", header.height);
//         let coin_id: CoinID = stdcode::deserialize(&hex::decode(coin_id)?)
//             .context("cannot deserialize hex coinid")?;
//         let (coin_data_height, full_proof) = self.client.get_coin(header, coin_id).await?;
//         Ok((coin_data_height, coin_id, full_proof))
//     }
//
//     pub async fn add_coin(
//         &mut self,
//         coin_id: &CoinID,
//         coin_data_height: &CoinDataHeight,
//     ) -> anyhow::Result<()> {
//         self.wallet_shell.insert_coin(*coin_id, coin_data_height.clone());
//         Ok(())
//     }
//
//     pub async fn create_tx(
//         &mut self,
//         dest_addr: &str,
//         amount: &str,
//         unit: &str,
//     ) -> anyhow::Result<Transaction> {
//         let number: u128 = amount.parse()?;
//         assert_eq!(unit, "TML");
//         let dest_addr = tmelcrypt::HashVal::from_addr(dest_addr)
//             .ok_or_else(|| anyhow::anyhow!("can't decode as address"))?;
//         let formatter = CoinData {
//             denom: DENOM_TMEL.to_vec(),
//             value: number * MICRO_CONVERTER,
//             covhash: dest_addr,
//         };
//         let outputs = vec![formatter.clone()];
//         let (header, _instant) = self.client.last_header().await?;
//         let fee_multiplier = header.fee_multiplier;
//
//         let tx = self
//             .wallet_shell
//             .pre_spend(outputs, fee_multiplier)?
//             .sign_ed25519(self.sk);
//
//         Ok(tx)
//     }
//
//     pub async fn send_tx(&mut self, to_send: Transaction) -> anyhow::Result<Transaction> {
//         eprintln!(">> Syncing state...");
//         self.client.broadcast_tx(to_send.clone()).await?;
//         eprintln!(">> Transaction {:?} broadcast!", to_send.hash_nosigs());
//         self.wallet_shell.spend(to_send.clone())?;
//         Ok(to_send)
//     }
//
//     pub async fn verify_tx(
//         &mut self,
//         tx: Transaction,
//     ) -> anyhow::Result<(Option<CoinDataHeight>, FullProof)> {
//         let header = self.client.last_header().await?.0;
//         let first_change = CoinID {
//             txhash: tx.hash_nosigs(),
//             index: 1,
//         };
//         Ok(self.client.get_coin(header, first_change).await?)
//     }
//
//     pub async fn get_spent_coins(&mut self) -> anyhow::Result<HashMap<CoinID, CoinDataHeight>> {
//         let mut spent_coins = HashMap::new();
//         for (coin_id, coin_data) in self.wallet_shell.spent_coins().iter() {
//             spent_coins.insert(*coin_id, coin_data.clone());
//         }
//         Ok(spent_coins)
//     }
//
//     pub async fn get_unspent_coins(&mut self) -> anyhow::Result<HashMap<CoinID, CoinDataHeight>> {
//         let mut unspent_coins = HashMap::new();
//         for (coin_id, coin_data) in self.wallet_shell.unspent_coins().iter() {
//             unspent_coins.insert(*coin_id, coin_data.clone());
//         }
//         Ok(unspent_coins)
//     }
//
//     pub async fn get_balance(&mut self) -> anyhow::Result<u128> {
//         let unspent_coins = self.get_unspent_coins().await?;
//         let mut total = 0;
//         for (_coin_id, coin_height) in unspent_coins.iter() {
//             total += coin_height.coin_data.value;
//         }
//         Ok(total)
//     }
//
//     pub async fn save(&mut self, wallet_name: &str) -> anyhow::Result<()> {
//         let encoded_data = stdcode::serialize(&self.wallet_shell).unwrap();
//         wallet_shell::update_by_name(&self.conn, &wallet_name, &encoded_data)
//             .expect("Failed to update wallet_shell");
//         Ok(())
//     }
// }
