use crate::client_protocol::*;
use crate::common::*;
use blkstructs::{melscript, CoinData, CoinDataHeight, CoinID, Header, Transaction, TxKind};
use collections::HashMap;
use rusqlite::{params, Connection, Result as SQLResult};
use std::net::SocketAddr;
use std::{collections, time::Instant};
use tmelcrypt::HashVal;

/// A network client with some in-memory caching.
pub struct Client {
    remote: SocketAddr,
    // cached variables
    last_header: Option<Header>,
    cache_date: Option<Instant>,
}

impl Client {
    /// Create a new network client.
    pub fn new(remote: SocketAddr) -> Self {
        Client {
            remote,
            last_header: None,
            cache_date: None,
        }
    }

    async fn sync_with_net(&mut self) -> anyhow::Result<()> {
        unimplemented!()
    }

    /// Obtain and verify the latest header.
    pub async fn last_header(&mut self) -> anyhow::Result<(Header, Instant)> {
        unimplemented!()
    }

    /// Get and verify a specific coin.
    pub async fn get_coin(
        &mut self,
        _header: Header,
        coin: CoinID,
    ) -> anyhow::Result<Option<CoinDataHeight>> {
        unimplemented!()
    }

    /// Get and verify a specific transaction at a specific height
    pub async fn get_tx(
        &mut self,
        height: u64,
        txhash: HashVal,
    ) -> anyhow::Result<(Option<Transaction>, autosmt::FullProof)> {
        unimplemented!()
    }

    /// Actually broadcast a transaction!
    pub async fn broadcast_tx(&mut self, tx: Transaction) -> anyhow::Result<bool> {
        unimplemented!()
    }
}
