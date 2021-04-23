use blkstructs::{CoinDataHeight, Transaction};

use crate::utils::context::ExecutionContext;
use crate::wallet::manager::WalletManager;
use crate::wallet::wallet::Wallet;

/// Responsible for executing a single client CLI command non-interactively.
pub struct CommandExecutor {
    context: ExecutionContext,
}

impl CommandExecutor {
    pub fn new(context: ExecutionContext) -> Self {
        Self { context }
    }

    /// Creates a new wallet, stores it into db and outputs the name & secret.
    pub async fn create_wallet(&self, wallet_name: &str) -> anyhow::Result<()> {
        let manager = WalletManager::new(self.context.clone());
        let wallet = manager.create_wallet(wallet_name).await?;
        let formatter = self.context.formatter.clone();
        formatter.wallet(wallet).await?;
        Ok(())
    }

    /// Creates a faucet tx to fund the wallet.
    /// It then sends the transaction and waits for a confirmation of the coins on the ledger.
    pub async fn faucet(
        &self,
        wallet_name: &str,
        secret: &str,
        amount: &str,
        unit: &str,
    ) -> anyhow::Result<()> {
        // Load wallet from wallet manager using name and secret
        let manager = WalletManager::new(self.context.clone());
        let wallet = manager.load_wallet(wallet_name, secret).await?;

        // Create faucet tx.
        let tx = wallet.create_faucet_tx(amount, unit, 1000000).await?;

        // Send the faucet tx.
        wallet.send_tx(&tx).await?;

        // Wait for tx confirmation
        let sleep_sec = self.context.sleep_sec;
        self.confirm_tx(&tx, &wallet, sleep_sec).await?;

        Ok(())
    }

    /// Sends coins from the wallet to a destination.
    pub async fn send_coins(
        &self,
        wallet_name: &str,
        secret: &str,
        _address: &str,
        _amount: &str,
        _unit: &str,
    ) -> anyhow::Result<()> {
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

        Ok(())
    }

    /// Adds coins by coin id to wallet.
    pub async fn add_coins(
        &self,
        _wallet_name: &str,
        _secret: &str,
        _coin_id: &str,
    ) -> anyhow::Result<()> {
        unimplemented!();
        // Ok(())
    }

    /// Shows the total known wallet balance.
    pub async fn show_balance(&self, _wallet_name: &str, _secret: &str) -> anyhow::Result<()> {
        unimplemented!();
        // Ok(())
    }

    /// Shows all the wallets by name that are stored in the db.
    pub async fn show_wallets(&self) -> anyhow::Result<()> {
        unimplemented!();
        // Ok(())
    }

    /// Check transaction until it is confirmed.
    /// TODO: we may need a max timeout to set upper bound on tx polling.
    pub async fn confirm_tx(
        &self,
        tx: &Transaction,
        wallet: &Wallet,
        sleep_sec: u64,
    ) -> anyhow::Result<CoinDataHeight> {
        loop {
            let (coin_data_height, coin_id) = wallet.check_tx(tx).await?;
            self.context
                .formatter
                .check_coin(&coin_data_height, &coin_id)
                .await?;
            self.context.sleep(sleep_sec).await?;
        }
    }
}
