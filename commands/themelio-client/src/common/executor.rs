use crate::common::context::ExecutionContext;
use crate::common::output;
use crate::wallet::manager::WalletManager;
use blkstructs::{Transaction, CoinDataHeight};
use crate::wallet::wallet::Wallet;

/// Responsible for common exeuction between interactive and non-interactive modes.
pub struct CommonExecutor {
    pub context: ExecutionContext,
}

impl CommonExecutor {
    pub fn new(context: ExecutionContext) -> Self {
        Self { context }
    }

    /// Creates a new wallet, stores it into db and outputs the name & secret.
    pub async fn create_wallet(&self, wallet_name: &str) -> anyhow::Result<()> {
        let manager = WalletManager::new(self.context.clone());
        let wallet = manager.create_wallet(wallet_name).await?;
        output::wallet(wallet).await?;
        Ok(())
    }

    /// Opens a wallet by name and secret and creates a faucet tx to fund the wallet.
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
        let fee = 2050000000;
        let tx = wallet.create_faucet_tx(amount, unit, fee).await?;

        // Send the faucet tx.
        wallet.send_tx(&tx).await?;

        // Wait for tx confirmation
        self.confirm_tx().await?;


        Ok(())
    }

    /// Check transaction until it is confirmed.
    /// TODO: we may need a timeout to set upper bound on tx polling.
    pub async fn confirm_tx(
        &self,
        tx: &Transaction,
        wallet: &Wallet,
        sleep_sec: u64,
    ) -> anyhow::Result<CoinDataHeight> {
        let sleep_time = 5;
        loop {
            let coin_data_height = wallet.check_tx(tx).await?;
            match coin_data_height {
                None => {
                    output::coin_pending().await;
                }
                Some(cdh) => {
                    output::coin_confirmed(&cdh).await;
                    return Ok(cdh)
                }
            }
            output::check_coin(coin_data_height).await?;
            if coin_data_height.is_some() {
                output::check_coin()
                // CommandOutput::print_check_tx(&coin_data_height).await?;
                println!("confirming");
                return Ok(coin_data_height.unwrap());
            }
            self.context.sleep(sleep_time).await?;
        }
    }

    /// Opens a wallet by name and secret and sends coins from the wallet to a destination.
    pub async fn send_coins(
        &self,
        wallet_name: &str,
        secret: &str,
        address: &str,
        amount: &str,
        unit: &str,
    ) -> anyhow::Result<()> {
        // Load wallet from wallet manager using name and secret
        let manager = WalletManager::new(self.context.clone());
        let wallet = manager.load_wallet(wallet_name, secret).await?;

        // TODO: while we don't ask for fee prompt in command mode we should do so in interactive mode
        // and an option type should be used somewhere here.

        // Create send mel tx.
        let fee = 2050000000;
        let tx = wallet.create_send_mel_tx(address, amount, unit, fee).await?;

        // Send the mel payment tx.
        wallet.send_tx(&tx).await?;

        // Wait for tx confirmation with a sleep time in seconds between polling.
        let sleep_sec = 2;
        let coin_data_height = self.confirm_tx(&tx, &wallet, sleep_sec).await?;

        // print confirmation results for send mel tx
        // println!("confirmed at height {:?}! ", coin_data_height);
        // CommandOutput::print_confirmed_send_mel_tx(&coin_data_height).await?;

        Ok(())
    }

    /// Adds coins by coin id to wallet.
    pub async fn add_coins(
        &self,
        wallet_name: &str,
        secret: &str,
        coin_id: &str,
    ) -> anyhow::Result<()> {
        unimplemented!();
        // Ok(())
    }

    /// Shows the total known wallet balance.
    pub async fn show_balance(&self, wallet_name: &str, secret: &str) -> anyhow::Result<()> {
        unimplemented!();
        // Ok(())
    }

    /// Shows all the wallets by name that are stored in the db.
    pub async fn show_wallets(&self) -> anyhow::Result<()> {
        unimplemented!();
        // Ok(())
    }

    /// Launch interactive mode until user exits.
    pub async fn shell(&self) -> anyhow::Result<()> {
        let runner = ShellRunner::new(self.context.clone());
        runner.run().await?;
        Ok(())
    }
}
