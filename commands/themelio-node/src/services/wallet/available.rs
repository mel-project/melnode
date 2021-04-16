// use crate::services::interactive::data::WalletData;
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
//         interactive::init(&conn).expect("Failed to load wallets");
//         AvailableWallets { conn }
//     }
//
//     /// Inserts a wallet_data into the database. If something already exists, returns true
//     pub fn insert(&self, wallet_name: &str, wallet_data: &WalletData) -> bool {
//         // If interactive already exists, do not insert and return true
//         let existing_wallet = interactive::read_by_name(&self.conn, &wallet_name);
//         if existing_wallet.is_ok() {
//             return true;
//         }
//
//         // Serialize interactive into encoded data and store it into db
//         let encoded_data = stdcode::serialize(&wallet_data).unwrap();
//         interactive::insert(&self.conn, &wallet_name, &encoded_data)
//             .expect("Failed to insert interactive data");
//         false
//     }
//
//     /// Gets a interactive with a certain name. If the interactive exists, return it; otherwise generate a fresh interactive.
//     pub fn get(&self, wallet_name: &str) -> Option<WalletData> {
//         // If interactive already exists, do not insert and return
//         let existing_wallet = interactive::read_by_name(&self.conn, &wallet_name);
//         if let Ok(interactive) = existing_wallet {
//             let interactive: WalletData = stdcode::deserialize(&interactive.encoded_data).unwrap();
//             return Some(interactive);
//         };
//         None
//     }
//
//     /// Get all wallets by name
//     pub fn get_all(&self) -> HashMap<String, WalletData> {
//         let existing_wallets = interactive::read_all(&self.conn).expect("Failed to get interactive records");
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
