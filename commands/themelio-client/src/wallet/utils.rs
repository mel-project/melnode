use crate::config::{BALLAST, FEE_MULTIPLIER};
use crate::wallet::error::WalletError;
use blkstructs::{CoinData, Transaction, TxKind, DENOM_TMEL, MICRO_CONVERTER};
use std::convert::TryInto;
use tmelcrypt::HashVal;

/// TODO: consider moving this into active wallet

/// Create a faucet transaction given inputs as strings amount, unit and a value for fee.
/// TODO: units variable is not yet used.
pub fn create_faucet_tx(
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
/// TODO: unit fix
pub fn create_send_mel_tx_outputs(
    dest_addr: &str,
    amount: &str,
    _unit: &str,
) -> anyhow::Result<Vec<CoinData>> {
    let value: u128 = amount.parse()?;
    let dest_addr = tmelcrypt::HashVal::from_addr(dest_addr)
        .ok_or_else(|| anyhow::anyhow!("can't decode as address"))?;

    let output = CoinData {
        denom: DENOM_TMEL.to_owned(),
        value: value * MICRO_CONVERTER,
        covhash: dest_addr,
        additional_data: vec![],
    };

    Ok(vec![output])
}

// Create deposit, withdraw, swap tx

/// Given a hex encoded string with the wallet secret return the Ed25519 secret key
pub fn get_secret_key(secret: &str) -> anyhow::Result<tmelcrypt::Ed25519SK> {
    let wallet_secret = hex::decode(secret)?;
    let wallet_secret = tmelcrypt::Ed25519SK(wallet_secret.as_slice().try_into()?);
    Ok(wallet_secret)
}
