use blkstructs::Transaction;
use std::{net::SocketAddr, sync::Arc};
mod node;
pub use node::*;

mod blksync;
mod staker;
pub(crate) use blksync::AbbreviatedBlock;
pub use staker::*;

mod client_protocol;
pub use client_protocol::*;
