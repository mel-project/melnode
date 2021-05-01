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
        .applied_fee(FEE_MULTIPLIER, BALLAST, 0);

        if tx.is_none() {
            anyhow::bail!(WalletError::InvalidTransactionArgs(
                "create faucet tx failed".to_string()
            ))
        }
        Ok(tx.unwrap())
    }

    /// Create a send mel tx
    /// TODO: add in optional fee as input arg
    pub async fn create_send_mel_tx(
        dest_addr: &str,
        amount: &str,
        _unit: &str,
    ) -> anyhow::Result<Transaction> {
        let value: u128 = amount.parse()?;
        let dest_addr = tmelcrypt::HashVal::from_addr(dest_addr)
            .ok_or_else(|| anyhow::anyhow!("can't decode as address"))?;

        let output = CoinData {
            denom: DENOM_TMEL.to_owned(),
            value: value * MICRO_CONVERTER,
            covhash: dest_addr,
            additional_data: vec![],
        };

        let tx = Transaction {
            kind: TxKind::Normal,
            inputs: vec![],
            outputs: vec![output],
            fee: 0,
            scripts: vec![],
            sigs: vec![],
            data: vec![],
        }
        .applied_fee(FEE_MULTIPLIER, BALLAST, 0);

        if tx.is_none() {
            anyhow::bail!(WalletError::InvalidTransactionArgs(
                "create send mel tx failed".to_string()
            ))
        }
        Ok(tx.unwrap())
    }

    // Create deposit, withdraw, swap tx
}
