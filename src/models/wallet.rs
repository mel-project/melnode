#[derive(Debug)]
pub struct WalletRecord {
    id: i32,
    wallet_name: Box<str>,
    encoded_data: Vec<u8>,
}

impl WalletRecord {
    /// Create a new wallet record from a wallet instance.
    pub fn new(encoded_data: Vec<u8>, wallet_name: &str) -> Self {
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
        let mut stmt = conn
            .prepare("SELECT id, wallet_name, encoded_data FROM wallet")
            .unwrap();
        let wallet_iter = stmt
            .query_map(params![], |row| {
                Ok(WalletRecord {
                    id: row.get(0)?,
                    wallet_name: row.get(1)?,
                    encoded_data: row.get(2)?,
                })
            })
            .unwrap();
        let mut wallets: HashMap<String, Wallet> = HashMap::new();
        for wallet_record in wallet_iter {
            let wr = wallet_record.unwrap();
            let wallet: Wallet = bincode::deserialize(&wr.encoded_data.clone()).unwrap();
            wallets.insert(wr.wallet_name.into_string(), wallet);
        }
        wallets
    }
}

#[cfg(test)]
mod tests {
    use crate::wallet::{Wallet, WalletRecord};
    use blkstructs;
    use im::hashmap;
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
            height: 0,
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
            height: 0,
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
