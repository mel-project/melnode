use crate::common::{snapshot_sleep, ExecutionContext};
use crate::io::CommandOutput;
use crate::shell::runner::ShellRunner;
use crate::wallet::manager::WalletManager;
use crate::wallet::wallet::Wallet;
use blkstructs::{CoinDataHeight, Transaction};

/// Responsible for executing a single client CLI command non-interactively.
pub struct CommandExecutor {
    pub context: ExecutionContext,
}

impl CommandExecutor {
    pub fn new(context: ExecutionContext) -> Self {
        Self { context }
    }

    /// Creates a new wallet, stores it into db and outputs the name & secret.
    pub async fn create_wallet(&self, wallet_name: &str) -> anyhow::Result<()> {
        let manager = WalletManager::new(self.context.clone());
        let wallet = manager.create_wallet(wallet_name).await?;
        CommandOutput::print_created_wallet(wallet).await?;
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

        // Wait for tx confirmation with a sleep time in seconds between polling.
        let sleep_sec = 2;
        let coin_data_height = self.confirm_tx(&tx, &wallet, sleep_sec).await?;

        // print confirmation results for faucet tx
        println!("confirmed at height {:?}! ", coin_data_height);
        // CommandOutput::print_confirmed_faucet_tx(&coin_data_height).await?;

        Ok(())
    }

    /// Update snapshot until we can confirm the transaction is at a certain height.
    // TODO: we need a timeout passed into this method or it should be called with a task race.
    pub async fn confirm_tx(
        &self,
        tx: &Transaction,
        wallet: &Wallet,
        sleep_sec: u64,
    ) -> anyhow::Result<CoinDataHeight> {
        loop {
            let coin_data_height = wallet.check_tx(tx).await?;
            if coin_data_height.is_some() {
                // CommandOutput::print_check_faucet_tx(&coin_data_height).await?;
                println!("confirming");
                return Ok(coin_data_height.unwrap());
            }
            snapshot_sleep(sleep_sec).await?;
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
        // // Load wallet from wallet manager using name and secret
        // let manager = WalletManager::new(&self.host.clone(), &self.database.clone());
        // let wallet = manager.load_wallet(wallet_name, secret).await?;
        //
        // // Create send coins tx.
        // let fee = 2050000000;
        // let tx = wallet.create_send_coins_tx(amount, unit, fee).await?;
        //
        // // Send the tx
        // wallet.send_tx(&tx).await?;
        //
        // // Wait for confirmation of the coin
        // let coin = CoinID {
        //     txhash: tx.hash_nosigs(),
        //     index: 0,
        // };
        // wallet.confirm_coins(&coin).await?;

        // print results

        // let shell = Wallet::new(&self.host, &self.database);
        // let wallet_data = shell.sub(&self.name, &self.secret).await?;
        // let prompt = sub::prompt::format_prompt(&self.version).await?;
        //                 let tx = active_wallet.create_tx(dest_addr, amount, unit).await?;
        //                 let fee_prompt = format!("Do you wish to send a tx with a fee of {} (y/n): ", tx.fee);
        //                 let fee_input = read_line(fee_prompt.to_string()).await.unwrap();
        //                 if !fee_input.contains('y') {
        //                     continue;
        //                 }

        //                 let tx = active_wallet.send_tx(tx).await?;
        //                 eprintln!(">> Sent tx.  Waiting to verify.");
        //                 loop {
        //                     let (coin_data_height, _proof) = active_wallet.verify_tx(tx.clone()).await?;
        //                     if let Some(out) = coin_data_height {
        //                         let their_coin = CoinID {
        //                             txhash: tx.hash_nosigs(),
        //                             index: 0,
        //                         };
        //                         let first_change = CoinID {
        //                             txhash: tx.hash_nosigs(),
        //                             index: 1,
        //                         };
        //                         eprintln!(">> Confirmed at height {}!", out.height);
        //                         eprintln!(
        //                             ">> CID (Sent) = {}",
        //                             hex::encode(stdcode::serialize(&their_coin).unwrap()).bold()
        //                         );
        //                         eprintln!(
        //                             ">> CID (Change) = {}",
        //                             hex::encode(stdcode::serialize(&first_change).unwrap()).bold()
        //                         );
        //                         break;
        //                     }
        //                 }

        Ok(())
    }

    /// Adds coins by coin id to wallet.
    pub async fn add_coins(
        &self,
        wallet_name: &str,
        secret: &str,
        coin_id: &str,
    ) -> anyhow::Result<()> {
        // let shell = Wallet::new(&self.host, &self.database);
        // let wallet_data = shell.sub(&self.name, &self.secret).await?;
        // let prompt = sub::prompt::format_prompt(&self.version).await?;
        //                 let (coin_data_height, coin_id, _full_proof) =
        //                     active_wallet.get_coin_data_by_id(coin_id).await?;
        //                 match coin_data_height {
        //                     None => {
        //                         eprintln!("Coin not found");
        //                         continue;
        //                     }
        //                     Some(coin_data_height) => {
        //                         eprintln!(
        //                             ">> Coin found at height {}! Added {} {} to data",
        //                             coin_data_height.height,
        //                             coin_data_height.coin_data.value,
        //                             {
        //                                 let val = coin_data_height.coin_data.denom.as_slice();
        //                                 format!("X-{}", hex::encode(val))
        //                             }
        //                         );
        //                         active_wallet.add_coin(&coin_id, &coin_data_height).await?;
        //                         eprintln!("Added coin to shell");
        //                     }
        //                 }
        Ok(())
    }

    /// Shows the total known wallet balance.
    pub async fn show_balance(&self, wallet_name: &str, secret: &str) -> anyhow::Result<()> {
        // let shell = Wallet::new(&self.host, &self.database);
        // let wallet_data = shell.sub(&self.name, &self.secret).await?;
        // let prompt = sub::prompt::format_prompt(&self.version).await?;
        //                 let balance = active_wallet.get_balance().await?;
        //                 eprintln!(">> **** BALANCE ****");
        //                 eprintln!(">> {}", balance);
        Ok(())
    }

    /// Shows all the wallets by name that are stored in the db.
    pub async fn show_wallets(&self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Launch shell mode until user exits.
    pub async fn shell(&self) -> anyhow::Result<()> {
        let runner = ShellRunner::new(self.context.clone());
        runner.run().await?;
        Ok(())
    }
}
