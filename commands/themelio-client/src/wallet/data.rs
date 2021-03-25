use blkstructs::{melvm, CoinData, CoinDataHeight, CoinID, Transaction, TxKind};
use serde::{Deserialize, Serialize};
use std::collections;
use tmelcrypt::{Ed25519PK, Ed25519SK, HashVal};

/// Immutable & cloneable in-memory data that can be persisted.
/// Does not store secrets!
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WalletData {
    unspent_coins: im::HashMap<CoinID, CoinDataHeight>,
    spent_coins: im::HashMap<CoinID, CoinDataHeight>,
    tx_in_progress: im::HashMap<HashVal, Transaction>,
    pub my_script: melvm::Covenant,
}

impl WalletData {
    /// Create a new data.
    pub fn new(my_script: melvm::Covenant) -> Self {
        WalletData {
            unspent_coins: im::HashMap::new(),
            spent_coins: im::HashMap::new(),
            tx_in_progress: im::HashMap::new(),
            my_script,
        }
    }

    /// Generates wallet data keypair and returns keypair and wallet data
    pub fn generate() -> (Ed25519SK, Ed25519PK, Self) {
        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let script = melvm::Covenant::std_ed25519_pk(pk);
        (sk, pk, WalletData::new(script))
    }

    /// Unspent Coins
    pub fn unspent_coins(&self) -> im::HashMap<CoinID, CoinDataHeight> {
        self.unspent_coins.clone()
    }

    /// Spent Coins
    pub fn spent_coins(&self) -> im::HashMap<CoinID, CoinDataHeight> {
        self.spent_coins.clone()
    }

    /// Inserts a coin into the data, returning whether or not the coin already exists.
    pub fn insert_coin(&mut self, coin_id: CoinID, coin_data_height: CoinDataHeight) -> bool {
        self.spent_coins.get(&coin_id).is_none()
            && self
            .unspent_coins
            .insert(coin_id, coin_data_height)
            .is_none()
    }

    /// Creates an **unsigned** transaction out of the coins in the data. Does not spend it yet.
    pub fn pre_spend(
        &self,
        outputs: Vec<CoinData>,
        fee_multiplier: u128,
    ) -> anyhow::Result<Transaction> {
        // find coins that might match
        let mut txn = Transaction {
            kind: TxKind::Normal,
            inputs: vec![],
            outputs,
            fee: 0,
            scripts: vec![self.my_script.clone()],
            data: vec![],
            sigs: vec![],
        };
        txn.fee = fee_multiplier.saturating_mul(txn.weight(1000));

        let output_sum = txn.total_outputs();
        let mut input_sum: collections::HashMap<Vec<u8>, u128> = collections::HashMap::new();
        for (coin, data) in self.unspent_coins.iter() {
            let existing_val = input_sum.get(&data.coin_data.denom).cloned().unwrap_or(0);
            if existing_val < output_sum.get(&data.coin_data.denom).cloned().unwrap_or(0) {
                txn.inputs.push(*coin);
                input_sum.insert(
                    data.coin_data.denom.clone(),
                    existing_val + data.coin_data.value,
                );
            }
        }
        // create change outputs
        let change = {
            let mut change = Vec::new();
            for (cointype, sum) in output_sum.iter() {
                let difference = input_sum
                    .get(cointype)
                    .unwrap_or(&0)
                    .checked_sub(*sum)
                    .ok_or_else(|| anyhow::anyhow!("not enough money"))?;
                if difference > 0 {
                    change.push(CoinData {
                        covhash: self.my_script.hash(),
                        value: difference,
                        denom: cointype.clone(),
                    })
                }
            }
            change
        };
        txn.outputs.extend(change.into_iter());
        assert!(txn.is_well_formed());
        Ok(txn)
    }

    /// Actually spend a transaction.
    pub fn spend(&mut self, txn: Transaction) -> anyhow::Result<()> {
        let mut oself = self.clone();
        // move coins from spent to unspent
        for input in txn.inputs.iter().cloned() {
            let coindata = oself
                .unspent_coins
                .remove(&input)
                .ok_or_else(|| anyhow::anyhow!("no such coin in data"))?;
            oself.spent_coins.insert(input, coindata);
        }
        // put tx in progress
        self.tx_in_progress.insert(txn.hash_nosigs(), txn);
        // "commit"
        *self = oself;
        Ok(())
    }
}
