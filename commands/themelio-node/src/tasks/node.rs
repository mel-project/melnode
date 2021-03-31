use std::collections::HashMap;

use crate::protocols::{NodeProtocol, StakerProtocol};
use crate::{config::VERSION, services::NodeStorage};
use blkstructs::{melvm, GenesisConfig, StakeDoc};
use smol::net::SocketAddr;
use structopt::StructOpt;
use tmelcrypt::{Ed25519SK, HashVal};
use tracing::instrument;
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
    staker_sk: Option<Ed25519SK>,

    /// Bootstrap addresses for the staker network.
    #[structopt(long)]
    staker_bootstrap: Vec<SocketAddr>,

    /// Listen address for the staker network.
    #[structopt(long)]
    staker_listen: Option<SocketAddr>,
}

/// Runs the main function for a node.
#[instrument(skip(opt))]
pub async fn run_node(opt: NodeConfig) {
    let _ = std::fs::create_dir_all(&opt.database);
    log::info!("themelio-core v{} initializing...", VERSION);
    log::info!("bootstrapping with {:?}", opt.bootstrap);
    // TODO: make this configurable rather than hardcoding the testnet
    let storage = NodeStorage::new(
        sled::open(&opt.database).unwrap(),
        GenesisConfig::std_testnet(),
    )
    .share();
    let _node_prot = NodeProtocol::new(opt.listen, opt.bootstrap.clone(), storage.clone());
    let _staker_prot = if let Some(my_sk) = opt.staker_sk {
        Some(
            StakerProtocol::new(
                opt.staker_listen.unwrap(),
                opt.staker_bootstrap.clone(),
                storage.clone(),
                my_sk,
            )
            .unwrap(),
        )
    } else {
        None
    };

    smol::future::pending::<()>().await;
}
