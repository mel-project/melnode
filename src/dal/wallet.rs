use rusqlite::{params, Connection, Error, Result};

pub struct WalletRecord {
    wallet_name: String,
    encoded_data: Vec<u8>,
}

/// Create a wallet record in db
pub fn insert(conn: &Connection, wallet_name: &str, encoded_data: &Vec<u8>) -> Result<()> {
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
        "INSERT INTO wallet (wallet_name, encoded_data) VALUES (?1, ?2)",
        params![wallet_name, encoded_data],
    )?;
    Ok(())
}

/// Read a wallet record from db using a wallet name
pub fn read_by_name(conn: &Connection, wallet_name: &str) -> Result<WalletRecord> {
    let mut stmt =
        conn.prepare("SELECT wallet_name, encoded_data FROM wallet WHERE wallet_name is (?1)")?;
    let mut wallet_iter = stmt.query_map(params![wallet_name], |row| {
        Ok(WalletRecord {
            wallet_name: row.get(0)?,
            encoded_data: row.get(1)?,
        })
    })?;
    wallet_iter.next().unwrap()
}

/// Read all wallet data records from db
pub fn read_all(conn: &Connection) -> Result<Vec<WalletRecord>> {
    let mut stmt = conn
        .prepare("SELECT wallet_name, encoded_data FROM wallet_data")
        .unwrap();
    let wallet_iter = stmt
        .query_map(params![], |row| {
            Ok(WalletRecord {
                wallet_name: row.get(0)?,
                encoded_data: row.get(1)?,
            })
        })
        .unwrap();
    wallet_iter.collect()
}
