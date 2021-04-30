use crate::config::{BALLAST, FEE_MULTIPLIER};
use crate::wallet::error::WalletError;
use blkstructs::{CoinData, Transaction, TxKind, DENOM_TMEL, MICRO_CONVERTER};
use tmelcrypt::HashVal;

pub struct TxBuilder;

impl TxBuilder {
    /// Create a faucet transaction given inputs as strings amount, unit and a value for fee.
    /// TODO: units variable is not yet used.
    pub async fn create_faucet_tx(
        amount: &str,
        _unit: &str,
        cov_hash: HashVal,
    ) -> anyhow::Result<Transaction> {
        let value: u128 = amount.parse()?;
        let fee_multiplier: u128 = FEE_MULTIPLIER;

        let tx = Transaction {
            kind: TxKind::Faucet,
            inputs: vec![],
            outputs: vec![CoinData {
                denom: DENOM_TMEL.to_owned(),
                covhash: cov_hash,
                value: value * MICRO_CONVERTER,
                additional_data: vec![],
            }],
            fee: 0,
            scripts: vec![],
            sigs: vec![],
            data: vec![],
        }
        .applied_fee(fee_multiplier, BALLAST, 0);

        if tx.is_none() {
            anyhow::bail!(WalletError::InvalidTransactionArgs(
                "create faucet tx failed".to_string()
            ))
        }
        Ok(tx.unwrap())
    }

    pub async fn create_send_mel_tx(
        addr: &str,
        amount: &str,
        unit: &str,
        fee: u128,
    ) -> anyhow::Result<Transaction> {
        unimplemented!()
        // let value: u128 = amount.parse()?;
        // let tx = Transaction {
        //     kind: TxKind::Faucet,
        //     inputs: vec![],
        //     outputs: vec![CoinData {
        //         denom: DENOM_TMEL.to_owned(),
        //         covhash: self.data.my_script.hash(),
        //         value: value * MICRO_CONVERTER,
        //     }],
        //     fee,
        //     scripts: vec![],
        //     sigs: vec![],
        //     data: vec![],
        // };
        // Ok(tx)
    }

    // Create deposit, withdraw, swap tx

    // create doscmint, stake tx
}
