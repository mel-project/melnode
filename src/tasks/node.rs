use crate::protocols::{NodeProtocol, StakerProtocol};
use crate::services::{insecure_testnet_keygen, SharedStorage, Storage};
use parking_lot::lock_api::RwLock;
use smol::net::SocketAddr;
use smol::Timer;
use std::sync::Arc;
use std::time::Duration;
use structopt::StructOpt;

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

/// Runs the main function for a node.
pub async fn run_node(opt: NodeConfig) {
    let _ = std::fs::create_dir_all(&opt.database);
    const VERSION: &str = "TMP";
    log::info!("themelio-core v{} initializing...", VERSION);
    log::info!("bootstrapping with {:?}", opt.bootstrap);
    let storage: SharedStorage =
        Arc::new(RwLock::new(Storage::open_testnet(&opt.database).unwrap()));
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
        Timer::after(Duration::from_secs(1)).await;
        {
            let storage = storage.clone();
            smol::unblock(move || storage.write().sync()).await;
        }
    }
}
