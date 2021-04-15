use crate::wallet::data::WalletData;
use tmelcrypt::Ed25519SK;
use blkstructs::{CoinID, TxKind, Transaction, CoinData, DENOM_TMEL, MICRO_CONVERTER, NetID};

use nodeprot::{ValClient, ValClientSnapshot};

use smol::Timer;
use std::time::Duration;
use crate::common::ExecutionContext;

/// Responsible for using an in memory wallet to send transactions.
pub struct Wallet {
    sk: Ed25519SK,
    pub name: String,
    pub data: WalletData,
    pub context: ExecutionContext
}
impl Wallet {
    pub fn new(sk: Ed25519SK, name: &str, data: WalletData, context: ExecutionContext) -> Self {
        let name = name.to_string();
        Self {
            sk,
            name,
            data,
            context,
        }
    }

    /// Create a faucet transaction given the amount, unit and a value for fee.
    pub async fn create_faucet_tx(&self, amount: &str, unit: &str, fee: u128) -> anyhow::Result<Transaction> {
        let value: u128 = amount.parse()?;
        let tx = Transaction {
            kind: TxKind::Faucet,
            inputs: vec![],
            outputs: vec![CoinData {
                denom: DENOM_TMEL.to_owned(),
                covhash: self.data.my_script.hash(),
                value: value * MICRO_CONVERTER,
            }],
            fee,
            scripts: vec![],
            sigs: vec![],
            data: vec![],
        };
        Ok(tx)
    }

    /// Update snapshot and send a transaction.
    pub async fn send_tx(&self, tx: &Transaction) -> anyhow::Result<()> {
        let snapshot = self.context.get_latest_snapshot().await?;
        let res = snapshot.raw.send_tx(tx.clone()).await;
        match res {
            Ok(_) => { println!("sent faucet tx"); }
            Err(ref err) => {
                println!("{:?}", err.clone())
            }
        }
        Ok(())
    }

    /// Update snapshot and confirm the transaction.
    pub async fn confirm_tx(&self, tx: &Transaction) -> anyhow::Result<()> {
        let coin = CoinID {
            txhash: tx.hash_nosigs(),
            index: 0,
        };
        loop {
            async fn sleep(dur: Duration) {
                Timer::after(dur).await;
            }
            sleep(Duration::from_secs(1)).await;
            let snapshot = self.context.get_latest_snapshot().await?;
            match snapshot.get_coin(coin).await? {
                None => {
                    println!("nothing");
                }
                Some(_) => {
                    println!("something");
                    break;
                }
            }
        }
        println!("transaction confirmed");
        // println!("{:?}", res);
        // query output state using tx hash
        // let tx_hash = tx.hash()
        // snapshot.get_coin(cid).await?;
        // SubShellOutput::faucet_tx(cid).await?;
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
        Ok(())
    }

//     /// Send coins to a recipient.
//     pub async fn send_coins(&self, mut wallet_data: &WalletData, dest: HashVal, amt: u128, denom: &[u8]) -> anyhow::Result<CoinID> {
//         Ok(CoinID{ txhash: Default::default(), index: 0 })
//     }
//
//     /// Add coins to this wallet
//     pub async fn add_coins(&self, wallet_data: &WalletData, ) -> anyhow::Result<CoinID> {
//         Ok(CoinID{ txhash: Default::default(), index: 0 })
//     }
//
//     /// Check the balance for this wallet.
//     pub async fn balance(&self, wallet_data: &WalletData, ) -> anyhow::Result<CoinID> {
//         Ok(CoinID{ txhash: Default::default(), index: 0 })
//     }

}