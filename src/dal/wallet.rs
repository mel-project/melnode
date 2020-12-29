use crate::dal::sql::SQLConnectionType;
use rusqlite::{params, Result as SQLResult};

/// Represents a single data record row element in db
#[derive(Debug)]
pub struct WalletRecord {
    id: i32,
    wallet_name: Box<str>,
    encoded_data: Vec<u8>,
}

/// WalletRecord data abstraction layer
pub struct WalletRecordDAL {
    conn: &'static SQLConnectionType,
}

impl WalletRecordDAL {
    pub fn new(conn: &SQLConnectionType) -> Self {
        return WalletRecordDAL { conn };
    }

    /// Create new data record or update if it exists
    pub fn upsert(&self, wallet_name: &str, encoded_data: Vec<u8>) -> SQLResult<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS data (
                  id              INTEGER PRIMARY KEY,
                  wallet_name     varchar(255) NOT NULL,
                  encoded_data    BLOB,
                  UNIQUE (wallet_name)
                  )",
            params![],
        )?;
        self.conn.execute(
            "INSERT INTO data (encoded_data) VALUES (?1, ?2)",
            params![wallet_name, encoded_data],
        )?;
        Ok(())
    }

    /// Load all data records
    pub fn read_all(&self) -> Vec<WalletRecord> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, wallet_name, encoded_data FROM data")
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
        let mut wallets: Vec<WalletRecord> = Vec::new();
        for wallet_record in wallet_iter {
            let wr = wallet_record.unwrap();
            wallets.push(wr);
        }
        wallets
    }
}

// TODO: refactor to match WalletDAL impl
// #[cfg(test)]
// mod tests {
//     use crate::data::{Wallet, WalletRecord};
//     use blkstructs;
//     use im::hashmap;
//     use rusqlite::{params, Connection, Result as SQLResult};
//
//     fn mock_create_wallet() -> Wallet {
//         // Create script
//         let (pk, sk) = tmelcrypt::ed25519_keygen();
//         let script = blkstructs::melscript::Script::std_ed25519_pk(pk);
//
//         // create unspent coins
//         let coin_id = blkstructs::CoinID {
//             txhash: tmelcrypt::HashVal([0; 32]),
//             index: 0,
//         };
//         let coin = blkstructs::CoinData {
//             conshash: script.hash(),
//             value: blkstructs::MICRO_CONVERTER * 1000,
//             cointype: blkstructs::COINTYPE_TMEL.to_owned(),
//         };
//         let coin_data_height = blkstructs::CoinDataHeight {
//             coin_data: coin,
//             height: 0,
//         };
//         let unspent_coins = hashmap! {
//             coin_id => coin_data_height
//         };
//         // create spent coins
//         let spent_coin_id = blkstructs::CoinID {
//             txhash: tmelcrypt::HashVal([0; 32]),
//             index: 0,
//         };
//         let spent_coin = blkstructs::CoinData {
//             conshash: script.hash(),
//             value: blkstructs::MICRO_CONVERTER * 1000,
//             cointype: blkstructs::COINTYPE_TMEL.to_owned(),
//         };
//         let spent_coin_data_height = blkstructs::CoinDataHeight {
//             coin_data: spent_coin,
//             height: 0,
//         };
//         let spent_coins = hashmap! {
//             spent_coin_id => spent_coin_data_height
//         };
//
//         let mut data = Wallet::new(script);
//         data.unspent_coins = unspent_coins;
//         data.spent_coins = spent_coins;
//
//         return data;
//     }
//
//     #[test]
//     fn store_wallet() -> SQLResult<()> {
//         // Create data record
//         let data = mock_create_wallet();
//         let wallet_name = "test";
//         let wallet_record = WalletRecord::new(data, &wallet_name);
//
//         // Insert data record
//         let conn = Connection::open_in_memory()?;
//         wallet_record.store(&conn);
//
//         // Verify that only one record inserted and
//         // it matches with the expected encoded data
//         let mut stmt = conn.prepare("SELECT id, wallet_name, encoded_data FROM data")?;
//         let wallet_iter = stmt.query_map(params![], |row| {
//             Ok(WalletRecord {
//                 id: row.get(0)?,
//                 wallet_name: row.get(1)?,
//                 encoded_data: row.get(2)?,
//             })
//         })?;
//
//         for (idx, data) in wallet_iter.enumerate() {
//             assert_eq!(wallet_record.encoded_data, data.unwrap().encoded_data);
//             assert_eq!(idx, 0);
//         }
//         Ok(())
//     }
// }
