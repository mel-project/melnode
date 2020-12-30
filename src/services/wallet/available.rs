use crate::dal::wallet;
use crate::services::wallet::data::WalletData;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;

pub struct AvailableWallets {
    conn: Connection,
}

impl AvailableWallets {
    pub fn new(path: &String) -> Self {
        let path = Path::new(path);
        let conn = Connection::open(path).expect("SQLite connection failure");
        wallet::init(&conn);
        AvailableWallets { conn }
    }

    /// Inserts a wallet_data into the database. If something already exists, returns true
    pub fn insert(&self, wallet_name: &str, wallet_data: &WalletData) -> bool {
        // If wallet already exists, do not insert and return true
        let existing_wallet = wallet::read_by_name(&self.conn, &wallet_name);
        if existing_wallet.is_ok() {
            return true;
        };

        // Serialize wallet into encoded data and store it into db
        let encoded_data = bincode::serialize(&wallet_data).unwrap();
        wallet::insert(&self.conn, &wallet_name, &encoded_data);
        return false;
    }

    /// Gets a wallet with a certain name. If the wallet exists, return it; otherwise generate a fresh wallet.
    pub fn get(&self, wallet_name: &str) -> Option<WalletData> {
        // If wallet already exists, do not insert and return
        let existing_wallet = wallet::read_by_name(&self.conn, &wallet_name);
        if let Ok(wallet) = existing_wallet {
            let wallet: WalletData = bincode::deserialize(&wallet.encoded_data).unwrap();
            return Some(wallet);
        };
        None
    }

    /// Get all wallets by name
    pub fn get_all(&self) -> HashMap<String, WalletData> {
        let existing_wallets = wallet::read_all(&self.conn).expect("Failed to get wallet records");
        let mut wallets_by_name = HashMap::new();
        for existing_wallet in existing_wallets {
            let wallet_name = existing_wallet.wallet_name.clone();
            let wallet_data: WalletData =
                bincode::deserialize(&existing_wallet.encoded_data).unwrap();
            wallets_by_name.insert(wallet_name, wallet_data);
        }
        wallets_by_name
    }
}
