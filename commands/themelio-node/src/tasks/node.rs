use std::path::PathBuf;

use crate::protocols::{NodeProtocol, StakerProtocol};
use crate::{config::VERSION, services::NodeStorage};
use anyhow::Context;
use blkstructs::GenesisConfig;
use smol::net::SocketAddr;
use structopt::StructOpt;
use tmelcrypt::{Ed25519SK, HashVal};
use tracing::instrument;
#[derive(Debug, StructOpt)]
pub struct NodeConfig {
    /// Listen address
    #[structopt(long)]
    listen: SocketAddr,

    /// Bootstrap addresses. May be given as a DNS name.
    #[structopt(long, default_value = "mainnet-bootstrap.themelio.org:11814")]
    bootstrap: Vec<String>,

    /// Database path
    #[structopt(long, default_value = "/var/themelio-node/blocks")]
    database: String,

    /// Specifies the secret key for staking.
    #[structopt(long)]
    staker_sk: Option<Ed25519SK>,

    /// Bootstrap addresses for the staker network.
    #[structopt(long)]
    staker_bootstrap: Vec<SocketAddr>,

    /// Listen address for the staker network.
    #[structopt(long)]
    staker_listen: Option<SocketAddr>,

    /// Payout address for staker rewards.
    #[structopt(long)]
    staker_payout_addr: Option<String>,

    /// If given, uses this TOML file to configure the network genesis rather than following the known testnet/mainnet genesis.
    #[structopt(long)]
    override_genesis: Option<PathBuf>,

    /// If set to true, default to the testnet. Otherwise, mainnet validation rules are used.
    #[structopt(long)]
    testnet: bool,

    /// Fee multiplier to target. Default is 1000.
    #[structopt(long, default_value = "1000")]
    target_fee_multiplier: u128,
}

/// Runs the main function for a node.
#[instrument(skip(opt))]
pub async fn run_node(opt: NodeConfig) -> anyhow::Result<()> {
    let _ = std::fs::create_dir_all(&opt.database);
    log::info!("themelio-core v{} initializing...", VERSION);
    let genesis = if let Some(path) = opt.override_genesis {
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
    let database = sled::Config::default()
        .path(&opt.database)
        .cache_capacity(1024 * 1024 * 100)
        .open()
        .context("can't open database")?;
    let storage = NodeStorage::new(database, genesis).share();
    let mut bootstrap = vec![];
    for name in opt.bootstrap.iter() {
        let addrs = smol::net::resolve(&name)
            .await
            .context("cannot resolve DNS bootstrap")?;
        bootstrap.extend(addrs);
    }
    log::info!("bootstrapping with {:?}", bootstrap);
    let _node_prot = NodeProtocol::new(netid, opt.listen, bootstrap, storage.clone());
    let _staker_prot = if let Some(my_sk) = opt.staker_sk {
        Some(
            StakerProtocol::new(
                opt.staker_listen.unwrap(),
                opt.staker_bootstrap.clone(),
                storage.clone(),
                my_sk,
                HashVal::from_addr(&opt.staker_payout_addr.unwrap())
                    .context("cannot parse payout address")?,
                opt.target_fee_multiplier,
            )
            .unwrap(),
        )
    } else {
        None
    };

    smol::future::pending().await
}
