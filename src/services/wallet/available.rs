use crate::dal::sql::SQL_SESSION;
use crate::dal::wallet;
use crate::services::wallet::data::WalletData;
use blkstructs::melscript;
use rusqlite::{Connection, Result as SQLResult};
use std::collections::HashMap;
use std::process::exit;
use std::sync::Arc;

pub struct AvailableWallets {}

impl AvailableWallets {
    pub fn new() -> AvailableWallets {
        AvailableWallets {}
    }

    /// Inserts a wallet_data into the database. If something already exists, returns true
    pub fn insert(&self, wallet_name: &str, wallet_data: WalletData) -> bool {
        // If wallet already exists, do not insert and return true
        let conn = Connection::open_in_memory().expect("SQLite connection failure");
        let existing_wallet = wallet::read_by_name(&conn, &wallet_name);
        if existing_wallet.is_err() {
            true
        }

        // Serialize wallet into encoded data and store it into db
        let encoded_data = bincode::serialize(&wallet_data).unwrap();
        wallet::insert(&conn, &wallet_name, &encoded_data);
        false
    }

    /// Gets a wallet with a certain name. If the wallet exists, return it; otherwise generate a fresh wallet.
    pub fn get_or_init(&self, wallet_name: &str) -> WalletData {
        // If wallet already exists, do not insert and return
        let conn = Connection::open_in_memory().expect("SQLite connection failure");
        let existing_wallet = wallet::read_by_name(&conn, &wallet_name);
        if let Ok(wallet) = existing_wallet {
            let wallet: WalletData = bincode::deserialize(&wallet.encoded_data).unwrap();
            wallet
        }

        // Otherwise create and return new wallet data
        WalletData::generate()
    }

    /// Get all wallets by name
    pub fn get_all(&self) -> HashMap<String, WalletData> {
        let conn = Connection::open_in_memory().expect("SQLite connection failure");
        let existing_wallets = wallet::read_all(&conn).expect("Failed to get wallet records");
        let mut wallets_by_name = HashMap::new();
        for &existing_wallet in existing_wallets {
            let wallet_data: WalletData =
                bincode::deserialize(&existing_wallet.encoded_data).unwrap();
            wallets_by_name.insert(existing_wallet.wallet_name, wallet_data)
        }
        wallets_by_name
    }
}
