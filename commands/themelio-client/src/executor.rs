use blkstructs::{CoinDataHeight, CoinID, Transaction};

use crate::context::ExecutionContext;
use crate::wallet::info::{CreatedWalletInfo, FaucetInfo, Printable};
use crate::wallet::manager::WalletManager;
use crate::wallet::wallet::ActiveWallet;

use crate::wallet::error::WalletError;
use crate::wallet::tx::TxBuilder;
use erased_serde::Serialize;

/// Responsible for executing a single client CLI command given all the inputs and returning a result.
pub struct CommandExecutor {
    context: ExecutionContext,
}

impl CommandExecutor {
    pub fn new(context: ExecutionContext) -> Self {
        Self { context }
    }

    /// Creates a new wallet, stores it into db and returns info about the created wallet.
    pub async fn create_wallet(&self, wallet_name: &str) -> anyhow::Result<CreatedWalletInfo> {
        // Create a wallet in storage and retrieve the active wallet
        let manager = WalletManager::new(self.context.clone());
        let wallet = manager.create_wallet(wallet_name).await?;

        // Return info on the created wallet.
        let info = CreatedWalletInfo {
            name: wallet.name().to_string(),
            address: wallet.data().my_covenant().hash().to_addr(),
            secret: hex::encode(wallet.secret().clone().0),
        };
        Ok(info)
    }

    /// Creates a faucet tx to fund the wallet.
    /// It then sends the transaction and waits for a confirmation of the coins on the ledger.
    pub async fn faucet(
        &self,
        wallet_name: &str,
        secret: &str,
        amount: &str,
        unit: &str,
    ) -> anyhow::Result<FaucetInfo> {
        // Load wallet from wallet manager using name and secret
        let manager = WalletManager::new(self.context.clone());
        let wallet = manager.load_wallet(wallet_name, secret).await?;

        // Create the faucet transaction and send it.
        let cov_hash = wallet.data().my_covenant().hash();
        let tx = TxBuilder::create_faucet_tx(amount, unit, cov_hash).await?;
        wallet.send_tx(&tx).await?;

        // Wait for confirmation of the transaction.
        let coin_data_height = self.confirm_tx(&tx, &wallet).await?;

        // Return information about the confirmed faucet transaction.
        let info = FaucetInfo {
            tx,
            coin_data_height,
        };
        Ok(info)
    }

    /// Sends coins from the wallet to a destination.
    pub async fn send_coins(
        &self,
        wallet_name: &str,
        secret: &str,
        _address: &str,
        _amount: &str,
        _unit: &str,
    ) -> anyhow::Result<Box<dyn Serialize>> {
        // Load wallet from wallet manager using name and secret
        let manager = WalletManager::new(self.context.clone());
        let _wallet = manager.load_wallet(wallet_name, secret).await?;

        // TODO: while we don't ask for fee prompt in command mode we should do so in wallet_shell mode
        // and an option type should be used somewhere here.

        // // Create send mel tx.
        // let fee = 2050000000;
        // let tx = wallet.create_send_mel_tx(address, amount, unit, fee).await?;
        //
        // // Send the mel payment tx.
        // wallet.send_tx(&tx).await?;
        //
        // // Wait for tx confirmation with a sleep time in seconds between polling.
        // let sleep_sec = 2;
        // let coin_data_height = self.confirm_tx(&tx, &wallet, sleep_sec).await?;

        // print confirmation results for send mel tx
        // println!("confirmed at height {:?}! ", coin_data_height);
        // CommandOutput::print_confirmed_send_mel_tx(&coin_data_height).await?;
        todo!("")
    }

    /// Adds coins by coin id to wallet.
    pub async fn add_coins(
        &self,
        _wallet_name: &str,
        _secret: &str,
        _coin_id: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Shows the total known wallet balance.
    pub async fn show_balance(&self, _wallet_name: &str, _secret: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// Shows all the wallets by name that are stored in the db.
    pub async fn show_wallets(&self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Liq. Deposit a token pair into melswap
    pub async fn deposit(
        &self,
        wallet_name: &str,
        secret: &str,
        cov_hash_a: &str,
        amount_a: &str,
        cov_hash_b: &str,
        amount_b: &str,
    ) -> anyhow::Result<Box<dyn Serialize>> {
        unimplemented!();
        // Ok(())
    }
    /// Liq. Deposit a token pair into melswap
    pub async fn withdraw(
        &self,
        wallet_name: &str,
        secret: &str,
        cov_hash_a: &str,
        amount_a: &str,
        cov_hash_b: &str,
        amount_b: &str,
    ) -> anyhow::Result<Box<dyn Serialize>> {
        unimplemented!();
        // Ok(())
    }
    /// Swap to and from mel
    pub async fn swap(
        &self,
        wallet_name: &str,
        secret: &str,
        cov_hash: &str,
        amount: &str,
    ) -> anyhow::Result<Box<dyn Serialize>> {
        unimplemented!();
        // Ok(())
    }

    /// Check transaction until it is confirmed.
    pub async fn confirm_tx(
        &self,
        tx: &Transaction,
        wallet: &ActiveWallet,
    ) -> anyhow::Result<CoinDataHeight> {
        loop {
            let (coin_data_height, coin_id) = wallet.check_sent_tx(tx).await?;
            // Move this logic to wallet
            // self.context
            //     .formatter
            //     .check_coin(coin_data_height.clone(), coin_id)
            //     .await?;
            if let Some(cdh) = coin_data_height {
                return Ok(cdh);
            }
            self.context.sleep(self.context.sleep_sec).await?;
        }
    }
}
