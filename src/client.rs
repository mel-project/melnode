use crate::client_protocol::*;
use crate::common::*;
use blkstructs::{melscript, CoinData, CoinDataHeight, CoinID, Header, Transaction, TxKind};
use std::net::SocketAddr;
use std::{collections, time::Instant};
use tmelcrypt::HashVal;
use serde::{Deserialize, Serialize};
use rusqlite::{params, Connection, Result as SQLResult};
use collections::HashMap;

/// A network client with some in-memory caching.
pub struct Client {
    remote: SocketAddr,
    // cached variables
    last_header: Option<Header>,
    cache_date: Option<Instant>,
}

impl Client {
    /// Create a new network client.
    pub fn new(remote: SocketAddr) -> Self {
        Client {
            remote,
            last_header: None,
            cache_date: None,
        }
    }

    async fn sync_with_net(&mut self) -> anyhow::Result<()> {
        // TODO: caching
        let route = self.remote;
        let header: Header = melnet::g_client()
            .request(route, TEST_ANET, "get_latest_header", 0u8)
            .await?;
        log::warn!("not validating consensus proof!");
        self.last_header = Some(header);
        self.cache_date = Some(Instant::now());
        Ok(())
    }

    /// Obtain and verify the latest header.
    pub async fn last_header(&mut self) -> anyhow::Result<(Header, Instant)> {
        self.sync_with_net().await?;
        Ok((self.last_header.unwrap(), self.cache_date.unwrap()))
    }

    /// Get and verify a specific coin.
    pub async fn get_coin(
        &mut self,
        _header: Header,
        coin: CoinID,
    ) -> anyhow::Result<Option<CoinDataHeight>> {
        let (hdr, _) = self.last_header().await?;
        let cdh: GetCoinResponse = melnet::g_client()
            .request(
                self.remote,
                TEST_ANET,
                "get_coin",
                GetCoinRequest {
                    coin_id: coin,
                    height: hdr.height,
                },
            )
            .await?;
        // TODO: validate branch
        Ok(cdh.coin_data)
    }

    /// Get and verify a specific transaction at a specific height
    pub async fn get_tx(
        &mut self,
        height: u64,
        txhash: HashVal,
    ) -> anyhow::Result<(Option<Transaction>, autosmt::FullProof)> {
        let response: GetTxResponse = melnet::g_client()
            .request(
                self.remote,
                TEST_ANET,
                "get_tx",
                GetTxRequest { txhash, height },
            )
            .await?;
        Ok((
            response.transaction,
            autosmt::CompressedProof(response.compressed_proof)
                .decompress()
                .ok_or_else(|| anyhow::anyhow!("could not decompress proof"))?,
        ))
    }

    /// Actually broadcast a transaction!
    pub async fn broadcast_tx(&mut self, tx: Transaction) -> anyhow::Result<bool> {
        let (_hdr, _) = self.last_header().await?;
        Ok(melnet::g_client()
            .request(self.remote, TEST_ANET, "newtx", tx)
            .await?)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// An immutable, cloneable in-memory wallet that can be synced to disk. Does not contain any secrets!
pub struct Wallet {
    unspent_coins: im::HashMap<CoinID, CoinDataHeight>,
    spent_coins: im::HashMap<CoinID, CoinDataHeight>,
    tx_in_progress: im::HashMap<HashVal, Transaction>,
    pub my_script: melscript::Script,
}

impl Wallet {
    /// Coins
    pub fn unspent_coins(&self) -> impl Iterator<Item = (&CoinID, &CoinDataHeight)> {
        self.unspent_coins.iter()
    }
    /// Create a new wallet.
    pub fn new(my_script: melscript::Script) -> Self {
        Wallet {
            unspent_coins: im::HashMap::new(),
            spent_coins: im::HashMap::new(),
            tx_in_progress: im::HashMap::new(),
            my_script,
        }
    }
    /// Inserts a coin into the wallet, returning whether or not the coin already exists.
    pub fn insert_coin(&mut self, coin_id: CoinID, coin_data_height: CoinDataHeight) -> bool {
        self.spent_coins.get(&coin_id).is_none()
            && self
                .unspent_coins
                .insert(coin_id, coin_data_height)
                .is_none()
    }

    /// Creates an **unsigned** transaction out of the coins in the wallet. Does not spend it yet.
    pub fn pre_spend(&self, outputs: Vec<CoinData>) -> anyhow::Result<Transaction> {
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
        let output_sum = txn.total_outputs();
        let mut input_sum: collections::HashMap<Vec<u8>, u64> = collections::HashMap::new();
        for (coin, data) in self.unspent_coins.iter() {
            let existing_val = input_sum
                .get(&data.coin_data.cointype)
                .cloned()
                .unwrap_or(0);
            if existing_val
                < output_sum
                    .get(&data.coin_data.cointype)
                    .cloned()
                    .unwrap_or(0)
            {
                txn.inputs.push(*coin);
                input_sum.insert(
                    data.coin_data.cointype.clone(),
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
                        conshash: self.my_script.hash(),
                        value: difference,
                        cointype: cointype.clone(),
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
                .ok_or_else(|| anyhow::anyhow!("no such coin in wallet"))?;
            oself.spent_coins.insert(input, coindata);
        }
        // put tx in progress
        self.tx_in_progress.insert(txn.hash_nosigs(), txn);
        // "commit"
        *self = oself;
        Ok(())
    }
}

#[derive(Debug)]
pub struct WalletRecord {
    id: i32,
    wallet_name: Box<str>,
    encoded_data: Vec<u8>,
}


impl WalletRecord {
    /// Create a new wallet record from a wallet instance.
    pub fn new(wallet: Wallet, wallet_name: &str) -> Self {
        let encoded_wallet = bincode::serialize(&wallet).unwrap();
        WalletRecord {
            id: 0,
            wallet_name: wallet_name.into(),
            encoded_data: encoded_wallet,
        }
    }

    // Store a wallet record
    pub fn store(&self, conn: &Connection) -> SQLResult<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS wallet (
                  id              INTEGER PRIMARY KEY,
                  wallet_name     varchar(255) NOT NULL,
                  encoded_data    BLOB,
                  UNIQUE (wallet_name)
                  )",
            params![],
        )?;
        conn.execute(
            "INSERT INTO wallet (encoded_data) VALUES (?1, ?2)",
            params![self.wallet_name, self.encoded_data.clone()],
        )?;
        Ok(())
    }

    pub fn load_all(conn: &Connection) -> HashMap<String, Wallet> {
        let mut stmt = conn.prepare("SELECT id, wallet_name, encoded_data FROM wallet").unwrap();
        let wallet_iter = stmt.query_map(params![], |row| {
            Ok(WalletRecord {
                id: row.get(0)?,
                wallet_name: row.get(1)?,
                encoded_data: row.get(2)?,
            })
        }).unwrap();
        let mut wallets: HashMap<String, Wallet> = HashMap::new();
        for (idx, wallet_record) in wallet_iter.enumerate() {
            let wr = wallet_record.unwrap();
            let wallet: Wallet = bincode::deserialize(&wr.encoded_data.clone()).unwrap();
            wallets.insert(wr.wallet_name.into_string(), wallet);
        }
        wallets
    }
}

#[cfg(test)]
mod tests {
    use im::hashmap;
    use blkstructs;
    use crate::client::{Wallet, WalletRecord};
    use rusqlite::{params, Connection, Result as SQLResult};

    fn mock_create_wallet() -> Wallet {
        // Create script
        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let script = blkstructs::melscript::Script::std_ed25519_pk(pk);

        // create unspent coins
        let coin_id = blkstructs::CoinID {
            txhash: tmelcrypt::HashVal([0; 32]),
            index: 0,
        };
        let coin = blkstructs::CoinData {
            conshash: script.hash(),
            value: blkstructs::MICRO_CONVERTER * 1000,
            cointype: blkstructs::COINTYPE_TMEL.to_owned(),
        };
        let coin_data_height = blkstructs::CoinDataHeight {
            coin_data: coin,
            height: 0
        };
        let unspent_coins = hashmap! {
            coin_id => coin_data_height
        };
        // create spent coins
        let spent_coin_id = blkstructs::CoinID {
            txhash: tmelcrypt::HashVal([0; 32]),
            index: 0,
        };
        let spent_coin = blkstructs::CoinData {
            conshash: script.hash(),
            value: blkstructs::MICRO_CONVERTER * 1000,
            cointype: blkstructs::COINTYPE_TMEL.to_owned(),
        };
        let spent_coin_data_height = blkstructs::CoinDataHeight {
            coin_data: spent_coin,
            height: 0
        };
        let spent_coins = hashmap! {
            spent_coin_id => spent_coin_data_height
        };

        let mut wallet = Wallet::new(script);
        wallet.unspent_coins = unspent_coins;
        wallet.spent_coins = spent_coins;

        return wallet;
    }

    #[test]
    fn store_wallet() -> SQLResult<()> {
        // Create wallet record
        let wallet = mock_create_wallet();
        let wallet_name = "test";
        let wallet_record = WalletRecord::new(wallet, &wallet_name);

        // Insert wallet record
        let conn = Connection::open_in_memory()?;
        wallet_record.store(&conn);

        // Verify that only one record inserted and
        // it matches with the expected encoded data
        let mut stmt = conn.prepare("SELECT id, wallet_name, encoded_data FROM wallet")?;
        let wallet_iter = stmt.query_map(params![], |row| {
            Ok(WalletRecord {
                id: row.get(0)?,
                wallet_name: row.get(1)?,
                encoded_data: row.get(2)?,
            })
        })?;

        for (idx, wallet) in wallet_iter.enumerate() {
            assert_eq!(wallet_record.encoded_data, wallet.unwrap().encoded_data);
            assert_eq!(idx, 0);
        }
        Ok(())
    }
}