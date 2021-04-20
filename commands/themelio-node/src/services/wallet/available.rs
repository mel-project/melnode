// use crate::services::wallet_shell::data::WalletData;
// use rusqlite::Connection;
// use std::collections::HashMap;
// use std::path::Path;
//
// pub struct AvailableWallets {
//     conn: Connection,
// }
//
// impl AvailableWallets {
//     pub fn new(path: &str) -> Self {
//         let path = Path::new(path);
//         let conn = Connection::sub(path).expect("SQLite connection failure");
//         wallet_shell::init(&conn).expect("Failed to load wallets");
//         AvailableWallets { conn }
//     }
//
//     /// Inserts a wallet_data into the database. If something already exists, returns true
//     pub fn insert(&self, wallet_name: &str, wallet_data: &WalletData) -> bool {
//         // If wallet_shell already exists, do not insert and return true
//         let existing_wallet = wallet_shell::read_by_name(&self.conn, &wallet_name);
//         if existing_wallet.is_ok() {
//             return true;
//         }
//
//         // Serialize wallet_shell into encoded data and store it into db
//         let encoded_data = stdcode::serialize(&wallet_data).unwrap();
//         wallet_shell::insert(&self.conn, &wallet_name, &encoded_data)
//             .expect("Failed to insert wallet_shell data");
//         false
//     }
//
//     /// Gets a wallet_shell with a certain name. If the wallet_shell exists, return it; otherwise generate a fresh wallet_shell.
//     pub fn get(&self, wallet_name: &str) -> Option<WalletData> {
//         // If wallet_shell already exists, do not insert and return
//         let existing_wallet = wallet_shell::read_by_name(&self.conn, &wallet_name);
//         if let Ok(wallet_shell) = existing_wallet {
//             let wallet_shell: WalletData = stdcode::deserialize(&wallet_shell.encoded_data).unwrap();
//             return Some(wallet_shell);
//         };
//         None
//     }
//
//     /// Get all wallets by name
//     pub fn get_all(&self) -> HashMap<String, WalletData> {
//         let existing_wallets = wallet_shell::read_all(&self.conn).expect("Failed to get wallet_shell records");
//         let mut wallets_by_name = HashMap::new();
//         for existing_wallet in existing_wallets {
//             let wallet_name = existing_wallet.wallet_name.clone();
//             let wallet_data: WalletData =
//                 stdcode::deserialize(&existing_wallet.encoded_data).unwrap();
//             wallets_by_name.insert(wallet_name, wallet_data);
//         }
//         wallets_by_name
//     }
// }
