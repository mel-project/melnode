use crate::utils::context::ExecutionContext;
use crate::wallet::data::WalletData;
use blkstructs::{
    CoinData, CoinDataHeight, CoinID, Transaction, TxKind, DENOM_TMEL, MICRO_CONVERTER,
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

    /// Create a faucet transaction given the amount, unit and a value for fee.
    pub async fn create_faucet_tx(
        &self,
        amount: &str,
        unit: &str,
        fee: u128,
    ) -> anyhow::Result<Transaction> {
        // TODO: units
        let value: u128 = amount.parse()?;
        let tx = Transaction {
            kind: TxKind::Faucet,
            inputs: vec![],
            outputs: vec![CoinData {
                denom: DENOM_TMEL.to_owned(),
                covhash: self.data.my_covenant().hash(),
                value: value * MICRO_CONVERTER,
                additional_data: vec![],
            }],
            fee,
            scripts: vec![],
            sigs: vec![],
            data: vec![],
        }
        .applied_fee(self.context.fee_multiplier().await?, 100);
        Ok(tx)
    }

    pub async fn create_send_mel_tx(
        &self,
        addr: &str,
        amount: &str,
        unit: &str,
        fee: u128,
    ) -> anyhow::Result<Transaction> {
        unimplemented!()
        // let value: u128 = amount.parse()?;
        // let tx = Transaction {
        //     kind: TxKind::Faucet,
        //     inputs: vec![],
        //     outputs: vec![CoinData {
        //         denom: DENOM_TMEL.to_owned(),
        //         covhash: self.data.my_script.hash(),
        //         value: value * MICRO_CONVERTER,
        //     }],
        //     fee,
        //     scripts: vec![],
        //     sigs: vec![],
        //     data: vec![],
        // };
        // Ok(tx)
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
