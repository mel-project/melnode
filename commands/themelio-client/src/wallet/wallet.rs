use crate::wallet::data::WalletData;
use tmelcrypt::Ed25519SK;
use blkstructs::{CoinID, TxKind, Transaction, CoinData, DENOM_TMEL, MICRO_CONVERTER};

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