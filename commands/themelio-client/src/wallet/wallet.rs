use crate::context::ExecutionContext;
use crate::wallet::data::WalletData;
use blkstructs::{CoinDataHeight, CoinID, Transaction};
use tmelcrypt::Ed25519SK;

use crate::config::{BALLAST, FEE_MULTIPLIER};
use crate::wallet::error::WalletError;
use anyhow::Context;
use blkstructs::melvm::Covenant;
use blkstructs::{CoinData, Denom, TxKind, MICRO_CONVERTER};
use colored::Colorize;
use tmelcrypt::HashVal;

/// Representation of an open wallet. Automatically keeps storage in sync.
pub struct ActiveWallet {
    sk: Ed25519SK,
    name: String,
    data: WalletData,
    context: ExecutionContext,
}

impl ActiveWallet {
    /// Creates a new wallet
    pub fn new(sk: Ed25519SK, name: &str, data: WalletData, context: ExecutionContext) -> Self {
        let name = name.to_string();
        Self {
            sk,
            name,
            data,
            context,
        }
    }

    /// Get name of the wallet
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the inner data of the wallet
    pub fn data(&mut self) -> &mut WalletData {
        &mut self.data
    }

    /// Get the secret key of the wallet
    pub fn secret(&self) -> &Ed25519SK {
        &self.sk
    }

    /// Send a faucet tx to this wallet, wait for confirmation and return results.
    pub async fn send_faucet_tx(
        &mut self,
        amount: &str,
        unit: &str,
    ) -> anyhow::Result<(CoinDataHeight, CoinID)> {
        // Create tx
        let cov_hash = self.data().my_covenant().hash();
        let tx = self.create_faucet_tx(amount, unit, cov_hash)?;
        eprintln!(
            "Created faucet transaction for {} mels with fee of {}",
            amount.bold(),
            tx.fee
        );

        // Send tx
        let snapshot = self.context.client.snapshot().await?;
        snapshot.get_raw().send_tx(tx.clone()).await?;
        eprintln!(">> Transaction {:?} broadcast!", tx.hash_nosigs());

        // Wait for confirmation of the transaction.
        let (coin_data_height, coin_id) = self.confirm_tx(&tx).await?;

        Ok((coin_data_height, coin_id))
    }

    pub async fn add_coins(&mut self, coin_id: &str) -> anyhow::Result<(CoinDataHeight, CoinID)> {
        let coin_id: CoinID = stdcode::deserialize(&hex::decode(coin_id)?)
            .context("cannot deserialize hex coin id")?;
        let snapshot = self.context.client.snapshot().await?;
        let coin_data_height = snapshot.get_coin(coin_id).await?;

        match coin_data_height {
            None => {
                eprintln!("Coin not found");
                anyhow::bail!(WalletError::CoinNotFound("".to_string()))
            }
            Some(coin_data_height) => {
                eprintln!(
                    ">> Coin found at height {}! Added {} {} to data",
                    coin_data_height.height,
                    coin_data_height.coin_data.value,
                    {
                        let val = coin_data_height.coin_data.denom.to_bytes();
                        format!("X-{}", hex::encode(val))
                    }
                );
                if self.data().insert_coin(coin_id, coin_data_height.clone()) {
                    eprintln!("Added coin to wallet");
                } else {
                    eprintln!("Coin already in wallet.");
                }

                Ok((coin_data_height, coin_id))
            }
        }
    }

    /// Send an amount of mel to a destination address, wait for confirmation and return results.
    pub async fn send_mel(
        &mut self,
        dest_addr: &str,
        amount: &str,
        unit: &str,
    ) -> anyhow::Result<(CoinDataHeight, CoinID)> {
        let outputs = self.create_send_mel_tx_outputs(dest_addr, amount, unit)?;
        eprintln!("Created outputs");
        let tx = self
            .data()
            .pre_spend(outputs, FEE_MULTIPLIER)?
            .signed_ed25519(self.sk);

        eprintln!(
            "Created send mel transaction for {} mels with fee of {}",
            amount.bold(),
            tx.fee
        );

        self.send_tx(&tx).await?;
        eprintln!("Sent transaction.");

        let (coin_data_height, coin_id) = self.confirm_tx(&tx).await?;
        eprintln!("Confirmed transaction.");

        Ok((coin_data_height, coin_id))
    }

    /// Update snapshot and send a transaction.
    async fn send_tx(&mut self, tx: &Transaction) -> anyhow::Result<()> {
        let snapshot = self.context.client.snapshot().await?;
        snapshot.get_raw().send_tx(tx.clone()).await?;
        eprintln!(">> Transaction {:?} broadcast!", tx.hash_nosigs());
        self.data().spend(tx.clone())?;
        Ok(())
    }

    /// Update snapshot and check if we can get the coin from the transaction.
    async fn check_sent_tx(
        &self,
        tx: &Transaction,
    ) -> anyhow::Result<(Option<CoinDataHeight>, CoinID)> {
        let coin = CoinID {
            txhash: tx.hash_nosigs(),
            index: 0,
        };
        let snapshot = self.context.client.snapshot().await?;
        Ok((snapshot.get_coin(coin).await?, coin))
    }

    /// Check the balance for this wallet.
    pub async fn balance(&mut self) -> anyhow::Result<()> {
        let unspent = self.data().unspent_coins();
        let mut total = 0;
        for (_coin_id, coin_height) in unspent.iter() {
            total += coin_height.coin_data.value;
        }

        eprintln!(">> **** BALANCE ****");
        eprintln!(">> {}", total);

        Ok(())
    }

    pub async fn deposit_tx(
        &mut self,
        coin_id_a: &str,
        amount_a: &str,
        coin_id_b: &str,
        amount_b: &str,
    ) -> anyhow::Result<()> {
        let coin_id_a: CoinID = stdcode::deserialize(&hex::decode(coin_id_a)?)
            .context("cannot deserialize hex coin id")?;
        let coin_id_b: CoinID = stdcode::deserialize(&hex::decode(coin_id_b)?)
            .context("cannot deserialize hex coin id")?;

        let snapshot = self.context.client.snapshot().await?;
        let coin_data_height_a = snapshot.get_coin(coin_id_a).await?;
        let coin_data_height_b = snapshot.get_coin(coin_id_b).await?;

        if coin_data_height_a.is_none() || coin_data_height_b.is_none() {
            anyhow::bail!(WalletError::InvalidTransactionArgs(
                "coin id does not exist".to_string()
            ))
        }

        let coin_data_height_a = coin_data_height_a.unwrap();
        let coin_data_height_b = coin_data_height_b.unwrap();

        let value_a: u128 = amount_a.parse()?;
        let value_b: u128 = amount_b.parse()?;

        let hash = coin_data_height_a.coin_data.covhash;

        let pk = self.sk.to_public();

        let cov = Covenant::std_ed25519_pk_legacy(pk);
        let cov_hash = cov.hash();

        let deposit_tx = Transaction {
            kind: TxKind::LiqDeposit,
            inputs: vec![coin_id_a, coin_id_b],
            outputs: vec![
                CoinData {
                    covhash: cov_hash,
                    value: value_a,
                    denom: coin_data_height_a.coin_data.denom,
                    additional_data: vec![],
                },
                CoinData {
                    covhash: cov_hash,
                    value: value_b,
                    denom: coin_data_height_b.coin_data.denom,
                    additional_data: vec![],
                },
            ],
            fee: 0,
            scripts: vec![Covenant::std_ed25519_pk_legacy(pk)],
            data: hash.to_vec(),
            sigs: vec![],
        }
        .applied_fee(FEE_MULTIPLIER, BALLAST, 0);

        let tx = match deposit_tx {
            None => {
                anyhow::bail!(WalletError::InvalidTransactionArgs(
                    "Invalid tx".to_string()
                ))
            }
            Some(tx) => tx,
        }
        .signed_ed25519(self.sk);

        self.send_tx(&tx).await?;

        Ok(())
    }

    pub async fn withdraw_tx(
        &mut self,
        coin_id_a: &str,
        amount_a: &str,
        coin_id_b: &str,
        amount_b: &str,
    ) -> anyhow::Result<()> {
        let coin_id_a: CoinID = stdcode::deserialize(&hex::decode(coin_id_a)?)
            .context("cannot deserialize hex coin id")?;
        let coin_id_b: CoinID = stdcode::deserialize(&hex::decode(coin_id_b)?)
            .context("cannot deserialize hex coin id")?;

        let snapshot = self.context.client.snapshot().await?;
        let coin_data_height_a = snapshot.get_coin(coin_id_a).await?;
        let coin_data_height_b = snapshot.get_coin(coin_id_b).await?;

        if coin_data_height_a.is_none() || coin_data_height_b.is_none() {
            anyhow::bail!(WalletError::InvalidTransactionArgs(
                "coin id does not exist".to_string()
            ))
        }

        let coin_data_height_a = coin_data_height_a.unwrap();
        let coin_data_height_b = coin_data_height_b.unwrap();

        let value_a: u128 = amount_a.parse()?;
        let value_b: u128 = amount_b.parse()?;

        let hash = coin_data_height_a.coin_data.covhash;

        let pk = self.sk.to_public();

        let cov = Covenant::std_ed25519_pk_legacy(pk);
        let cov_hash = cov.hash();

        let withdraw_tx = Transaction {
            kind: TxKind::LiqWithdraw,
            inputs: vec![coin_id_a, coin_id_b],
            outputs: vec![
                CoinData {
                    covhash: cov_hash,
                    value: value_a,
                    denom: coin_data_height_a.coin_data.denom,
                    additional_data: vec![],
                },
                CoinData {
                    covhash: cov_hash,
                    value: value_b,
                    denom: coin_data_height_b.coin_data.denom,
                    additional_data: vec![],
                },
            ],
            fee: 0,
            scripts: vec![Covenant::std_ed25519_pk_legacy(pk)],
            data: hash.to_vec(),
            sigs: vec![],
        }
        .applied_fee(FEE_MULTIPLIER, BALLAST, 0);

        let tx = match withdraw_tx {
            None => {
                anyhow::bail!(WalletError::InvalidTransactionArgs(
                    "Invalid tx".to_string()
                ))
            }
            Some(tx) => tx,
        }
        .signed_ed25519(self.sk);

        self.send_tx(&tx).await?;

        Ok(())
    }

    /// Check transaction until it is confirmed and output progress to std err.
    async fn confirm_tx(&self, tx: &Transaction) -> anyhow::Result<(CoinDataHeight, CoinID)> {
        eprint!("Waiting for transaction confirmation.");
        loop {
            let (coin_data_height, coin_id) = self.check_sent_tx(tx).await?;
            if let Some(cd_height) = coin_data_height {
                eprintln!();
                eprintln!(
                    ">>> Coin is confirmed at current height {}",
                    cd_height.height
                );
                return Ok((cd_height, coin_id));
            }
            eprint!(".");
            self.context.sleep(self.context.sleep_sec).await?;
        }
    }

    /// Create a faucet transaction given inputs as strings amount, unit and a value for fee.
    /// TODO: units variable is not yet used.
    fn create_faucet_tx(
        &self,
        amount: &str,
        _unit: &str,
        cov_hash: HashVal,
    ) -> anyhow::Result<Transaction> {
        let value: u128 = amount.parse()?;

        let tx = Transaction {
            kind: TxKind::Faucet,
            inputs: vec![],
            outputs: vec![CoinData {
                denom: Denom::Mel,
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
    fn create_send_mel_tx_outputs(
        &self,
        dest_addr: &str,
        amount: &str,
        _unit: &str,
    ) -> anyhow::Result<Vec<CoinData>> {
        let value: u128 = amount.parse()?;
        let dest_addr = tmelcrypt::HashVal::from_addr(dest_addr)
            .ok_or_else(|| anyhow::anyhow!("can't decode as address"))?;

        let output = CoinData {
            denom: Denom::Mel,
            value: value * MICRO_CONVERTER,
            covhash: dest_addr,
            additional_data: vec![],
        };

        Ok(vec![output])
    }
}
