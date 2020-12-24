use blkstructs::Transaction;
use std::{net::SocketAddr, sync::Arc};
mod node;
pub use node::*;

mod blksync;
mod staker;
pub use staker::*;
