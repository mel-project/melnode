#![feature(try_blocks)]

//! [Themelio](https://themelio.org) is a work-in-progress public blockchain focused on security, performance, and long-term stability

mod protocols;
use main_anet_client::{run_anet_client, AnetClientConfig};
use protocols::{NodeProtocol, StakerProtocol};
mod common;
use common::*;
mod storage;
use parking_lot::RwLock;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
pub use storage::*;
mod client;
mod main_anet_client;
use structopt::StructOpt;
use tracing::instrument;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, StructOpt)]
pub enum Config {
    /// Runs a network node (auditor/stakeholder) that serves clients and other nodes.
    Node(NodeConfig),
    /// Runs a thin client that connects to other nodes.
    AnetClient(AnetClientConfig),
}

#[derive(Debug, StructOpt)]
pub struct NodeConfig {
    /// Listen address
    #[structopt(long)]
    listen: SocketAddr,

    /// Bootstrap addresses
    #[structopt(long)]
    bootstrap: Vec<SocketAddr>,

    /// Test spam
    #[structopt(long)]
    test_spam: bool,

    /// Database path
    #[structopt(long, default_value = "/tmp/testnet")]
    database: String,

    /// Testnet type
    #[structopt(long)]
    test_stakeholder: Option<usize>,

    /// Listen address for the staker network.
    #[structopt(long)]
    listen_staker: Option<SocketAddr>,
}

#[instrument(skip(opt))]
pub async fn run_main(opt: Config) {
    match opt {
        Config::Node(cfg) => run_node(cfg).await,
        Config::AnetClient(cfg) => run_anet_client(cfg).await,
    }
}

/// Runs the main function for a node.
#[instrument(skip(opt))]
async fn run_node(opt: NodeConfig) {
    let _ = std::fs::create_dir_all(&opt.database);

    log::info!("themelio-core v{} initializing...", VERSION);
    log::info!("bootstrapping with {:?}", opt.bootstrap);
    let storage = Arc::new(RwLock::new(Storage::open_testnet(&opt.database).unwrap()));
    let _node_prot = NodeProtocol::new(opt.listen, opt.bootstrap.clone(), storage.clone()).unwrap();
    let _staker_prot = if let Some(v) = opt.test_stakeholder {
        let my_sk = insecure_testnet_keygen(v).1;
        Some(
            StakerProtocol::new(
                opt.listen_staker.unwrap(),
                opt.bootstrap.clone(),
                storage.clone(),
                my_sk,
            )
            .unwrap(),
        )
    } else {
        None
    };

    // Storage syncer
    loop {
        Timer::after(Duration::from_secs(30)).await;
        {
            let storage = storage.clone();
            smol::unblock(move || storage.write().sync()).await;
        }
    }
}
