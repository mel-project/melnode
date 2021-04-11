use std::{collections::HashMap, path::PathBuf};

use crate::protocols::{NodeProtocol, StakerProtocol};
use crate::{config::VERSION, services::NodeStorage};
use anyhow::Context;
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

    /// If given, uses this TOML file to configure the genesis state rather than the default testnet.
    #[structopt(long)]
    genesis_config: Option<PathBuf>,
}

/// Runs the main function for a node.
#[instrument(skip(opt))]
pub async fn run_node(opt: NodeConfig) -> anyhow::Result<()> {
    let _ = std::fs::create_dir_all(&opt.database);
    log::info!("themelio-core v{} initializing...", VERSION);
    log::info!("bootstrapping with {:?}", opt.bootstrap);
    // TODO: make this configurable rather than hardcoding the testnet
    let genesis = if let Some(path) = opt.genesis_config {
        let genesis_toml = smol::fs::read(&path)
            .await
            .context("cannot read genesis config")?;
        toml::from_slice(&genesis_toml)?
    } else {
        GenesisConfig::std_testnet()
    };
    let storage = NodeStorage::new(
        sled::open(&opt.database).context("cannot open database")?,
        genesis,
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

    smol::future::pending().await
}
