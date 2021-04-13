use crate::wallet::data::WalletData;
use tmelcrypt::Ed25519SK;

/// Responsible for using an in memory wallet to send transactions.
pub struct Wallet {
    pub(crate) sk: Ed25519SK,
    pub(crate) name: String,
    pub(crate) data: WalletData
}

impl Wallet {
    pub fn new(sk: Ed25519SK, name: &str, data: WalletData) -> Self {
        let name = name.to_string();
        Self {
            sk,
            name,
            data
        }
    }

    pub async fn faucet_transaction(wallet_name, amount, unit) -> anyhow::Result<()> {
        // ["faucet", number, unit] => {
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
        Ok(())
    }
//
//     /// Use faucet to mint mels.
//     pub async fn faucet(&self, mut wallet_data: &WalletData, amt: u128, denom: &[u8] ) -> anyhow::Result<CoinID> {
//
//         // let shell = Wallet::new(&self.host, &self.database);
//         //
//         // let wallet_data = shell.sub(&self.name, &self.secret).await?;
//         //
//         // let coin = shell.faucet(&wallet_data, self.amt, self.denom).await?;
//         //
//         // prompter::output_faucet_tx(wallet_data, coin).await?;
//         //
//         // self.confirm_faucet_tx(coin).await?;
//         //
//         // prompter::faucet_tx_confirmed().await?;
//         Ok(CoinID{ txhash: Default::default(), index: 0 })
//     }
//
//     async fn confirm_faucet(&self, _coin_id: CoinID) -> anyhow::Result<()> {
//         // loop {
//         //
//         //     prompter::faucet_tx_confirming().await?;
//         // }
//         //                 eprintln!(
// //                     ">> Faucet transaction for {} mels broadcast!",
// //                     number.to_string().bold()
// //                 );
// //                 eprintln!(">> Waiting for confirmation...");
// //                 // loop until we get coin data height and proof from last header
// //                 loop {
// //                     let (coin_data_height, _hdr) = active_wallet.get_coin_data(coin).await?;
// //                     if let Some(cd_height) = coin_data_height {
// //                         eprintln!(
// //                             ">>> Coin is confirmed at current height {}",
// //                             cd_height.height
// //                         );
//
// //                         eprintln!(
// //                             ">> CID = {}",
// //                             hex::encode(stdcode::serialize(&coin).unwrap()).bold()
// //                         );
// //                         break;
// //                     }
//         Ok(())
//     }
//
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