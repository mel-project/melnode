use blkstructs::{CoinDataHeight, CoinID, Transaction};

use crate::context::ExecutionContext;
use crate::wallet::info::{
    BalanceInfo, CoinsInfo, CreatedWalletInfo, DepositInfo, FaucetInfo, Printable, SendCoinsInfo,
    SwapInfo, WalletsInfo, WithdrawInfo,
};
use crate::wallet::manager::WalletManager;
use crate::wallet::wallet::ActiveWallet;

use crate::config::FEE_MULTIPLIER;
use crate::wallet::utils::{create_faucet_tx, create_send_mel_tx_outputs, get_secret_key};
use colored::Colorize;
use std::collections::BTreeMap;

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

    /// Creates a faucet tx to fund the wallet and sends it.
    /// It waits for a confirmation of the coins on the ledger.
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
        let tx = create_faucet_tx(amount, unit, cov_hash)?;
        eprintln!(
            "Created faucet transaction for {} mels with fee of {}",
            amount.bold(),
            tx.fee
        );

        wallet.send_tx(&tx).await?;
        eprintln!("Sent transaction.");

        // Wait for confirmation of the transaction.
        let (coin_data_height, coin_id) = self.confirm_tx(&tx, &wallet).await?;

        // Return information about the confirmed faucet transaction.
        let info = FaucetInfo {
            coin_id,
            coin_data_height,
        };

        Ok(info)
    }

    /// Adds coins by coin id to a wallet.
    pub async fn add_coins(
        &self,
        _wallet_name: &str,
        _secret: &str,
        _coin_id: &str,
    ) -> anyhow::Result<CoinsInfo> {
        Ok(CoinsInfo)
    }

    /// Sends coins from a wallet to an address.
    /// TODO: consider an optional fee arg for testing tips
    pub async fn send_coins(
        &self,
        wallet_name: &str,
        secret: &str,
        address: &str,
        amount: &str,
        unit: &str,
    ) -> anyhow::Result<SendCoinsInfo> {
        // Load wallet from wallet manager using name and secret
        let manager = WalletManager::new(self.context.clone());
        let wallet = manager.load_wallet(wallet_name, secret).await?;

        // Create send mel tx.
        let secret = get_secret_key(secret)?;
        let outputs = create_send_mel_tx_outputs(address, amount, unit)?;
        let tx = wallet
            .data()
            .pre_spend(outputs, FEE_MULTIPLIER)?
            .signed_ed25519(secret);
        eprintln!(
            "Created send mel transaction for {} mels with fee of {}",
            amount.bold(),
            tx.fee
        );

        wallet.send_tx(&tx).await?;
        eprintln!("Sent transaction.");

        // Wait for confirmation of the transaction.
        let (coin_data_height, coin_id) = self.confirm_tx(&tx, &wallet).await?;

        let info = SendCoinsInfo {
            coin_data_height,
            coin_id,
        };
        Ok(info)
    }

    /// Shows the total known wallet balance.
    pub async fn show_balance(
        &self,
        _wallet_name: &str,
        _secret: &str,
    ) -> anyhow::Result<BalanceInfo> {
        Ok(BalanceInfo)
    }

    /// Shows all the wallets by name that are stored in the db.
    pub async fn show_wallets(&self) -> anyhow::Result<WalletsInfo> {
        // Get all wallets in storage by name
        let manager = WalletManager::new(self.context.clone());
        let wallets = manager.get_all_wallets().await?;

        // Create info on wallets and return it
        let wallet_addrs_by_name = wallets
            .into_iter()
            .map(|(k, v)| (k, v.my_covenant().hash().to_addr()))
            .collect::<BTreeMap<String, String>>();
        let info = WalletsInfo {
            wallet_addrs_by_name,
        };

        Ok(info)
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
    ) -> anyhow::Result<DepositInfo> {
        Ok(DepositInfo)
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
    ) -> anyhow::Result<WithdrawInfo> {
        Ok(WithdrawInfo)
    }
    /// Swap to and from mel
    pub async fn swap(
        &self,
        wallet_name: &str,
        secret: &str,
        cov_hash: &str,
        amount: &str,
    ) -> anyhow::Result<SwapInfo> {
        Ok(SwapInfo)
    }

    /// Check transaction until it is confirmed and output progress to std err.
    pub async fn confirm_tx(
        &self,
        tx: &Transaction,
        wallet: &ActiveWallet,
    ) -> anyhow::Result<(CoinDataHeight, CoinID)> {
        eprint!("Waiting for transaction confirmation.");
        loop {
            let (coin_data_height, coin_id) = wallet.check_sent_tx(tx).await?;
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
}
