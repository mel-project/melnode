mod args;
mod blkidx;
#[cfg(feature = "metrics")]
mod loki;
#[cfg(feature = "metrics")]
mod prometheus;
mod protocols;
#[cfg(feature = "metrics")]
mod public_ip_address;
mod storage;

#[cfg(feature = "metrics")]
use crate::prometheus::{AWS_INSTANCE_ID, AWS_REGION};

#[cfg(feature = "metrics")]
use tokio::runtime::Runtime;

use crate::protocols::{NodeProtocol, StakerProtocol};
use crate::storage::NodeStorage;

use args::Args;
use once_cell::sync::Lazy;
use structopt::StructOpt;
use tracing::instrument;

#[cfg(feature = "metrics")]
pub static RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().expect("Could not create tokio runtime."));

#[cfg(unix)]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[instrument]
fn main() -> anyhow::Result<()> {
    // Create a background thread which checks for deadlocks every 10s
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(10));
        let deadlocks = parking_lot::deadlock::check_deadlock();
        if deadlocks.is_empty() {
            continue;
        }

        log::error!("{} deadlocks detected", deadlocks.len());
        for (i, threads) in deadlocks.iter().enumerate() {
            log::error!("Deadlock #{}", i);
            for t in threads {
                log::error!("Thread Id {:#?}", t.thread_id());
                log::error!("{:#?}", t.backtrace());
            }
        }
    });

    env_logger::Builder::from_env("RUST_LOG")
        .parse_filters("themelio_node=debug,warn,novasymph")
        .init();
    let opts = Args::from_args();

    smolscale::block_on(main_async(opts))
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Runs the main function for a node.
#[instrument(skip(opt))]
pub async fn main_async(opt: Args) -> anyhow::Result<()> {
    #[cfg(not(feature = "metrics"))]
    log::info!("themelio-core v{} initializing...", VERSION);

    #[cfg(feature = "metrics")]
    log::info!(
        "hostname={} public_ip={} network={} region={} instance_id={} themelio-core v{} initializing...",
        crate::prometheus::HOSTNAME.as_str(),
        crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
        crate::prometheus::NETWORK.read().expect("Could not get a read lock on NETWORK."),
        AWS_REGION.read().expect("Could not get a read lock on AWS_REGION"),
        AWS_INSTANCE_ID.read().expect("Could not get a read lock on AWS_INSTANCE_ID"),
        VERSION
    );
    let genesis = opt.genesis_config().await?;
    let netid = genesis.network;
    let storage: NodeStorage = opt.storage().await?;
    let bootstrap = opt.bootstrap().await?;
    #[cfg(not(feature = "metrics"))]
    log::info!("bootstrapping with {:?}", bootstrap);
    #[cfg(feature = "metrics")]
    log::info!(
        "hostname={} public_ip={} network={} region={} instance_id={} bootstrapping with {:?}",
        crate::prometheus::HOSTNAME.as_str(),
        crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
        crate::prometheus::NETWORK.read().expect("Could not get a read lock on NETWORK."),
        AWS_REGION.read().expect("Could not get a read lock on AWS_REGION"),
        AWS_INSTANCE_ID.read().expect("Could not get a read lock on AWS_INSTANCE_ID"),
        bootstrap
    );
    let _node_prot = NodeProtocol::new(
        netid,
        opt.listen_addr(),
        opt.advertise_addr(),
        bootstrap,
        storage.clone(),
        opt.index_coins,
    );
    let _staker_prot = if let Some((
        staker_sk,
        staker_listen,
        staker_bootstrap,
        target_fee_multiplier,
        staker_payout_addr,
    )) = opt.staker_sk().await?
    {
        Some(StakerProtocol::new(
            staker_listen,
            staker_bootstrap,
            storage.clone(),
            staker_sk,
            staker_payout_addr,
            target_fee_multiplier,
        )?)
    } else {
        None
    };

    #[cfg(feature = "metrics")]
    crate::prometheus::GLOBAL_STORAGE
        .set(storage)
        .ok()
        .expect("Could not write to GLOBAL_STORAGE");

    #[cfg(feature = "metrics")]
    std::thread::spawn(|| RUNTIME.block_on(crate::prometheus::prometheus()));

    #[cfg(feature = "metrics")]
    std::thread::spawn(|| RUNTIME.block_on(crate::loki::loki()));

    #[cfg(feature = "metrics")]
    std::thread::spawn(|| RUNTIME.block_on(crate::prometheus::run_aws_information()));

    smol::future::pending().await
}