use std::path::PathBuf;

use crate::protocols::{NodeProtocol, StakerProtocol};
use crate::{config::VERSION, services::NodeStorage};
use anyhow::Context;
use blkstructs::GenesisConfig;
use smol::net::SocketAddr;
use structopt::StructOpt;
use tmelcrypt::Ed25519SK;
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
    #[structopt(long, default_value = "/tmp/themelio-mainnet")]
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

    /// If given, uses this TOML file to configure the genesis state rather than the default mainnet.
    #[structopt(long)]
    genesis_config: Option<PathBuf>,

    /// If set to true, default to the testnet
    #[structopt(long)]
    testnet: bool,
}

/// Runs the main function for a node.
#[instrument(skip(opt))]
pub async fn run_node(opt: NodeConfig) -> anyhow::Result<()> {
    let _ = std::fs::create_dir_all(&opt.database);
    log::info!("themelio-core v{} initializing...", VERSION);
    log::info!("bootstrapping with {:?}", opt.bootstrap);
    let genesis = if let Some(path) = opt.genesis_config {
        let genesis_toml = smol::fs::read(&path)
            .await
            .context("cannot read genesis config")?;
        toml::from_slice(&genesis_toml)?
    } else if opt.testnet {
        GenesisConfig::std_testnet()
    } else {
        GenesisConfig::std_mainnet()
    };
    let netid = genesis.network;
    let storage = NodeStorage::new(
        sled::open(&opt.database).context("cannot open database")?,
        genesis,
    )
    .share();
    let _node_prot = NodeProtocol::new(netid, opt.listen, opt.bootstrap.clone(), storage.clone());
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

    smol::future::pending().await
}
