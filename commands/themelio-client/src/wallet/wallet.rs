use crate::context::ExecutionContext;
use crate::wallet::data::WalletData;
use blkstructs::{
    CoinDataHeight, CoinID, Transaction,
};
use tmelcrypt::Ed25519SK;

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

    /// Update snapshot and send a transaction.
    pub async fn send_tx(&self, tx: &Transaction) -> anyhow::Result<()> {
        let snapshot = self.context.client.snapshot().await?;
        snapshot.get_raw().send_tx(tx.clone()).await?;
        Ok(())
    }

    /// Update snapshot and check if we can get the coin from the transaction.
    pub async fn check_sent_tx(
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

    //     /// Send coins to a recipient.
    //     pub async fn send_coins(&self, mut wallet_data: &WalletData, dest: HashVal, amt: u128, denom: &[u8]) -> anyhow::Result<CoinID> {
    //         Ok(CoinID{ txhash: Default::default(), index: 0 })
    //     }
    //
    //     /// Add coins to this wallet
    //     pub async fn add_coins(&self, wallet_data: &WalletData, ) -> anyhow::Result<CoinID> {
    //         Ok(CoinID{ txhash: Default::default(), index: 0 })
    //     }
    //
    //     /// Check the balance for this wallet.
    //     pub async fn balance(&self, wallet_data: &WalletData, ) -> anyhow::Result<CoinID> {
    //         Ok(CoinID{ txhash: Default::default(), index: 0 })
    //     }

    /// Get name of the wallet
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the inner data of the wallet
    pub fn data(&self) -> &WalletData {
        &self.data
    }

    /// Get the secret key of the wallet
    pub fn secret(&self) -> &Ed25519SK {
        &self.sk
    }
}
