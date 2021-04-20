use crate::wallet::wallet::Wallet;
use std::collections::BTreeMap;
use crate::wallet::data::WalletData;
use blkstructs::{CoinDataHeight, CoinID};

pub trait OutputFormatter {

    /// Display wallet information.
    fn wallet(&self, wallet: Wallet) -> anyhow::Result<()>;

    /// Display information on all stored wallets by name.
    fn wallet_addresses_by_name(&self, wallets: BTreeMap<String, WalletData>) -> anyhow::Result<()>;

    /// Display message showing height and coin id information upon a coin being confimed.
    fn coin_confirmed(&self, coin_data_height: &CoinDataHeight, coin: &CoinID) -> anyhow::Result<()>;

    /// Display message that coin is not yet confirmed.
    fn coin_pending(&self) -> anyhow::Result<()>;

    /// Display function which displays pending message until a coin is confirmed
    /// at which a confirmed message will be displayed.
    /// Typically can be used to wrap pending and confirming messages.
    fn check_coin(&self, coin_data_height: &Option<CoinDataHeight>, coin_id: &CoinID) -> anyhow::Result<()>;
}
