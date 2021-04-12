// use crate::services::shell::data::WalletData;
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
//         shell::init(&conn).expect("Failed to load wallets");
//         AvailableWallets { conn }
//     }
//
//     /// Inserts a wallet_data into the database. If something already exists, returns true
//     pub fn insert(&self, wallet_name: &str, wallet_data: &WalletData) -> bool {
//         // If shell already exists, do not insert and return true
//         let existing_wallet = shell::read_by_name(&self.conn, &wallet_name);
//         if existing_wallet.is_ok() {
//             return true;
//         }
//
//         // Serialize shell into encoded data and store it into db
//         let encoded_data = stdcode::serialize(&wallet_data).unwrap();
//         shell::insert(&self.conn, &wallet_name, &encoded_data)
//             .expect("Failed to insert shell data");
//         false
//     }
//
//     /// Gets a shell with a certain name. If the shell exists, return it; otherwise generate a fresh shell.
//     pub fn get(&self, wallet_name: &str) -> Option<WalletData> {
//         // If shell already exists, do not insert and return
//         let existing_wallet = shell::read_by_name(&self.conn, &wallet_name);
//         if let Ok(shell) = existing_wallet {
//             let shell: WalletData = stdcode::deserialize(&shell.encoded_data).unwrap();
//             return Some(shell);
//         };
//         None
//     }
//
//     /// Get all wallets by name
//     pub fn get_all(&self) -> HashMap<String, WalletData> {
//         let existing_wallets = shell::read_all(&self.conn).expect("Failed to get shell records");
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
