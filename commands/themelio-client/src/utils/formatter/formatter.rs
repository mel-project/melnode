use crate::wallet::data::WalletData;
use crate::wallet::wallet::ActiveWallet;
use async_trait::async_trait;
use blkstructs::{CoinDataHeight, CoinID};
use std::collections::BTreeMap;

#[async_trait]
pub trait OutputFormatter {
    /// Display wallet information.
    async fn wallet(&self, wallet: ActiveWallet) -> anyhow::Result<()>;

    /// Display information on all stored wallets by name.
    async fn wallet_addresses_by_name(
        &self,
        wallets: BTreeMap<String, WalletData>,
    ) -> anyhow::Result<()>;

    /// Display message showing height and coin id information upon a coin being confimed.
    async fn coin_confirmed(
        &self,
        coin_data_height: &CoinDataHeight,
        coin: &CoinID,
    ) -> anyhow::Result<()>;

    /// Display message that coin is not yet confirmed.
    async fn coin_pending(&self) -> anyhow::Result<()>;

    /// Display function which displays pending message until a coin is confirmed
    /// at which a confirmed message will be displayed.
    /// Typically can be used to wrap pending and confirming messages.
    async fn check_coin(
        &self,
        coin_data_height: &Option<CoinDataHeight>,
        coin_id: &CoinID,
    ) -> anyhow::Result<()>;
}
