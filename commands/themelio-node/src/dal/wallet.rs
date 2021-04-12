use rusqlite::{params, Connection, Result};

#[derive(Debug)]
pub struct WalletRecord {
    pub id: i32,
    pub wallet_name: String,
    pub encoded_data: Vec<u8>,
}

/// Create shell schema table if it doesn't already exist
pub fn init(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS shell (
              id              INTEGER PRIMARY KEY,
              wallet_name     varchar(255) NOT NULL,
              encoded_data    BLOB,
              UNIQUE (wallet_name)
              )",
        params![],
    )?;
    Ok(())
}

/// Create a shell record in db
pub fn insert(conn: &Connection, wallet_name: &str, encoded_data: &[u8]) -> Result<()> {
    let wr = WalletRecord {
        id: 0,
        wallet_name: String::from(wallet_name),
        encoded_data: encoded_data.into(),
    };
    conn.execute(
        "INSERT INTO shell (wallet_name, encoded_data) VALUES (?1, ?2)",
        params![wr.wallet_name, wr.encoded_data],
    )?;
    Ok(())
}

/// Update a shell record in db
pub fn update_by_name(conn: &Connection, wallet_name: &str, encoded_data: &[u8]) -> Result<()> {
    conn.execute(
        "UPDATE shell SET encoded_data=(?2) where wallet_name=(?1)",
        params![wallet_name, encoded_data],
    )?;
    Ok(())
}

/// Read a shell record from db using a shell name
pub fn read_by_name(conn: &Connection, wallet_name: &str) -> anyhow::Result<WalletRecord> {
    let mut stmt =
        conn.prepare("SELECT id, wallet_name, encoded_data FROM shell WHERE wallet_name is (?1)")?;
    let mut wallet_iter = stmt.query_map(params![wallet_name], |row| {
        let wr = WalletRecord {
            id: row.get(0)?,
            wallet_name: row.get(1)?,
            encoded_data: row.get(2)?,
        };
        Ok(wr)
    })?;
    Ok(wallet_iter
        .next()
        .ok_or_else(|| anyhow::anyhow!("Couldn't find a record"))??)
}

/// Read all shell data records from db
pub fn read_all(conn: &Connection) -> Result<Vec<WalletRecord>> {
    let mut stmt = conn
        .prepare("SELECT id, wallet_name, encoded_data FROM shell")
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
    wallet_iter.collect()
}
