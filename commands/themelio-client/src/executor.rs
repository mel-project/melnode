use blkstructs::{CoinID, Transaction};

use crate::context::ExecutionContext;
use crate::wallet::info::{
    BalanceInfo, CoinsInfo, CreatedWalletInfo, DepositInfo, FaucetInfo, Printable, SendCoinsInfo,
    SwapInfo, WalletsInfo, WithdrawInfo,
};
use crate::wallet::manager::WalletManager;

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
        let mut wallet = manager.create_wallet(wallet_name).await?;

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
        let mut wallet = manager.load_wallet(wallet_name, secret).await?;

        // Create the faucet transaction and send it.
        let (coin_data_height, coin_id) = wallet.send_faucet_tx(amount, unit).await?;

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
        wallet_name: &str,
        secret: &str,
        coin_id: &str,
    ) -> anyhow::Result<CoinsInfo> {
        // Load wallet from wallet manager using name and secret
        let manager = WalletManager::new(self.context.clone());
        let mut wallet = manager.load_wallet(wallet_name, secret).await?;

        // Add the coins
        let (coin_data_height, coin_id) = wallet.add_coins(coin_id).await?;

        // Save the wallet state
        manager
            .save_wallet(wallet_name, wallet.data().clone())
            .await?;

        // Return the information about the added coins
        let info = CoinsInfo {
            coin_data_height,
            coin_id,
        };
        Ok(info)
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
        let mut wallet = manager.load_wallet(wallet_name, secret).await?;

        // Create send mel tx.
        let (coin_data_height, coin_id) = wallet.send_mel(address, amount, unit).await?;

        // Save the wallet state
        manager
            .save_wallet(wallet_name, wallet.data().clone())
            .await?;

        // Return info about sent coins
        let info = SendCoinsInfo {
            coin_data_height,
            coin_id,
        };
        Ok(info)
    }

    /// Shows the total known wallet balance.
    pub async fn show_balance(
        &self,
        wallet_name: &str,
        secret: &str,
    ) -> anyhow::Result<BalanceInfo> {
        // Load wallet from wallet manager using name and secret
        let manager = WalletManager::new(self.context.clone());
        let mut wallet = manager.load_wallet(wallet_name, secret).await?;

        wallet.balance().await?;

        // Save the wallet state
        manager
            .save_wallet(wallet_name, wallet.data().clone())
            .await?;

        Ok(BalanceInfo)
    }

    /// Shows all the wallets by name that are stored in the db.
    pub async fn show_wallets(&self) -> anyhow::Result<WalletsInfo> {
        // Get all wallet addresses in storage by wallet name name
        let manager = WalletManager::new(self.context.clone());
        let wallet_addresses_by_name = manager.wallet_addresses_by_name().await?;

        // Return information on stored wallets
        let info = WalletsInfo {
            wallet_addrs_by_name: wallet_addresses_by_name,
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
}
