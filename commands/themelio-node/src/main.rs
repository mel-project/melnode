mod args;
#[cfg(feature = "metrics")]
mod prometheus;
mod protocols;
#[cfg(feature = "metrics")]
mod public_ip_address;
mod storage;

use crate::protocols::{NodeProtocol, StakerProtocol};
use crate::storage::NodeStorage;

use args::Args;
#[cfg(feature = "metrics")]
use async_compat::Compat;
use structopt::StructOpt;
use tracing::instrument;

#[cfg(unix)]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[instrument]
fn main() -> anyhow::Result<()> {
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
        "hostname={} public_ip={} themelio-core v{} initializing...",
        crate::prometheus::HOSTNAME.as_str(),
        crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
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
        "hostname={} public_ip={} bootstrapping with {:?}",
        crate::prometheus::HOSTNAME.as_str(),
        crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
        bootstrap
    );
    let _node_prot = NodeProtocol::new(
        netid,
        opt.listen_addr(),
        opt.advertise_addr(),
        bootstrap,
        storage.clone(),
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
    Compat::new(crate::prometheus::prometheus()).await;

    smol::future::pending().await
}
