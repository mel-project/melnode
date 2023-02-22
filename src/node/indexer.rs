use std::{net::SocketAddr, time::Duration};

use anyhow::Context;
use melblkidx::Indexer;
use melprot::Client;

use themelio_structs::{Checkpoint, NetID};

use crate::storage::Storage;

pub struct WrappedIndexer {
    indexer: Indexer,
    _task: smol::Task<()>,
}

impl WrappedIndexer {
    /// Creates a new CoinIndexer.
    pub async fn start(
        network: NetID,
        storage: Storage,
        connect_addr: SocketAddr,
    ) -> anyhow::Result<Self> {
        let mut localhost_listen_addr = connect_addr;
        localhost_listen_addr.set_ip("127.0.0.1".parse().unwrap());
        // TODO: connect_lazy shouldn't return a Result, since backhaul.connect_lazy is "infallible"?
        let client = Client::connect_http(network, localhost_listen_addr).await?;
        let _task = smolscale::spawn(indexer_loop(storage.clone(), client.clone()));
        Ok(Self {
            indexer: Indexer::new(storage.get_indexer_path(), client)
                .context("indexer failed to be created")?,
            _task,
        })
    }

    /// Gets a reference to the indexer within.
    pub fn inner(&self) -> &Indexer {
        &self.indexer
    }
}

async fn indexer_loop(storage: Storage, client: Client) {
    loop {
        let trusted_height = storage.highest_state().await;
        client.trust(Checkpoint {
            height: trusted_height.header().height,
            header_hash: trusted_height.header().hash(),
        });
        smol::Timer::after(Duration::from_secs(1)).await;
    }
}
