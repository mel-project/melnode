use rusqlite::{Connection, Error, Result};
use smol::lock::RwLock;
use std::sync::Arc;

pub type SQLConnectionType = Arc<RwLock<rusqlite::Result<Connection, Error>>>;

// TODO: Make this load from disk using env variables
pub const SQL_CONNECTION: SQLConnectionType = Arc::new(RwLock::new(Connection::open_in_memory()));
