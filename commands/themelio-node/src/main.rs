use std::{net::SocketAddr, path::PathBuf, time::Duration};

use anyhow::Context;
use structopt::StructOpt;
mod protocols;
mod storage;

use themelio_stf::GenesisConfig;
use tmelcrypt::{Ed25519SK, HashVal};
use tracing::instrument;

use crate::{
    protocols::{NodeProtocol, StakerProtocol},
    storage::NodeStorage,
};

// #[cfg(unix)]
// #[global_allocator]
// static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[instrument]
fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env("RUST_LOG")
        .parse_filters("themelio_node=debug,warn")
        .init();
    let opts = Args::from_args();

    // Create a background thread which checks for deadlocks
    std::thread::spawn(check_deadlock);

    smolscale::block_on(main_async(opts))
}

fn check_deadlock() {
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let deadlocks = parking_lot::deadlock::check_deadlock();
        if deadlocks.is_empty() {
            continue;
        }

        println!("{} deadlocks detected", deadlocks.len());
        for (i, threads) in deadlocks.iter().enumerate() {
            println!("Deadlock #{}", i);
            for t in threads {
                println!("Thread Id {:#?}", t.thread_id());
                println!("{:#?}", t.backtrace());
            }
        }
    }
}

#[derive(Debug, StructOpt)]
pub struct Args {
    /// Listen address
    #[structopt(long)]
    listen: SocketAddr,

    /// Bootstrap addresses. May be given as a DNS name.
    #[structopt(long, default_value = "mainnet-bootstrap.themelio.org:11814")]
    bootstrap: Vec<String>,

    /// Database path
    #[structopt(long, default_value = "/var/themelio-node/main.sqlite3")]
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

    /// Reset last block to the given height.
    #[structopt(long)]
    emergency_reset_block: Option<u64>,
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Runs the main function for a node.
#[instrument(skip(opt))]
pub async fn main_async(opt: Args) -> anyhow::Result<()> {
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
    let database = boringdb::Database::open(&opt.database)?;
    log::debug!("database opened at {}", opt.database);

    let storage = NodeStorage::new(database, genesis).share();

    // Reset block. This is used to roll back history in emergencies
    if let Some(height) = opt.emergency_reset_block {
        let mut storage = storage.write();
        log::warn!("*** EMERGENCY RESET TO BLOCK {} ***", height);
        let history = storage.history_mut();
        while history
            .get_tips()
            .iter()
            .any(|f| dbg!(f.header().height) > height)
        {
            history.delete_tips();
        }
    }

    log::debug!("node storage opened");
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
                    .context("cannot parse payout address")?
                    .into(),
                opt.target_fee_multiplier,
            )
            .unwrap(),
        )
    } else {
        None
    };

    smol::future::pending().await
}
