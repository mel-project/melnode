use rusqlite::{Connection, Error, Result};
use std::sync::Arc;

// TODO: Make this load from disk using env variables
pub const SQL_SESSION: Arc<dyn Fn() -> Result<Connection>> =
    Arc::new(|| Connection::open_in_memory());
