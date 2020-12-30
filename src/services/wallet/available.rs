use crate::dal::sql::SQL_SESSION;
use crate::dal::wallet;
use crate::services::wallet::data::WalletData;
use blkstructs::melscript;
use rusqlite::{Connection, Result as SQLResult};
use std::collections::HashMap;
use std::sync::Arc;

pub struct AvailableWallets {
    active_wallet: Option<WalletData>,
    session: Arc<dyn Fn() -> SQLResult<Connection>>,
}

impl AvailableWallets {
    pub fn new() -> AvailableWallets {
        AvailableWallets {
            active_wallet: None,
            session: SQL_SESSION.clone(),
        }
    }

    // pub fn get_active() -> Option<WalletData> {}
    //
    // // Returns a result type?
    // pub fn set_active(wallet_name: &str) {}

    /// Inserts a wallet_data into the database. If something already exists, returns true
    pub fn insert(&self, wallet_name: &str, wallet_data: WalletData) -> bool {
        // If wallet already exists, do not insert and return true
        let session: SQLResult<Connection> = self.session();
        let conn = session.expect("SQLite connection failure");
        let existing_wallet = wallet::read_by_name(&conn, &wallet_name);
        if existing_wallet.is_err() {
            true
        }

        // Serialize wallet into encoded data and store it into db
        let encoded_data = bincode::serialize(&wallet_data).unwrap();
        wallet::insert(&conn, &wallet_name, &encoded_data);
        false
    }

    // // /// Gets a wallet with a certain name. If the wallet exists, return it; otherwise generate a fresh wallet.
    // // pub fn get_or_init(&self, wallet_name: &str) -> WalletData {
    // //     // If wallet already exists, do not insert and return
    // //     let conn = Connection::open_in_memory().expect("SQLite connection failure");
    // //     let existing_wallet = wallet::read_by_name(&conn, &wallet_name);
    // //     if existing_wallet.is_err() {
    // //         None
    // //     }
    // // }
    //
    // pub fn unlock() {
    //     // if let Some(wallet) = wallets.get(&wallet_name.to_string()) {
    //     //     let wallet_secret = hex::decode(wallet_secret)?;
    //     //     let wallet_secret =
    //     //         tmelcrypt::Ed25519SK(wallet_secret.as_slice().try_into()?);
    //     //     if melscript::Script::std_ed25519_pk(wallet_secret.to_public())
    //     //         != wallet.my_script
    //     //     {
    //     //         Err(anyhow::anyhow!(
    //     //             "unlocking failed, make sure you have the right secret!"
    //     //         ))?;
    //     //     }
    //     //     current_wallet = Some((wallet_name.to_string(), wallet_secret));
    //     //     prompt_stack.push(format!("({})", wallet_name).yellow().to_string());
    //     // }
    // }
    //
    // pub fn list() -> Vec<WalletData> {
    //     // let connection = Connection::open_in_memory()?;
    //     // let wallet_records = wallet::read_all(&connection);
    //     // let mut wallets = Vec::new();
    //     // for wallet_record in &wallet_records.iter() {}
    //     // vec![]
    //     // writeln!(tw, ">> [NAME]\t[ADDRESS]")?;
    //     // for (name, wallet) in wallets.iter() {
    //     //     writeln!(tw, ">> {}\t{}", name, wallet.my_script.hash().to_addr())?;
    //     // }
    // }
}
