mod args;
#[cfg(feature = "metrics")]
mod prometheus;
#[cfg(feature = "metrics")]
mod public_ip_address;
mod protocols;
mod storage;

use crate::protocols::{NodeProtocol, StakerProtocol};
use crate::storage::SharedStorage;

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
        .parse_filters("themelio_node=debug,warn")
        .init();
    let opts = Args::from_args();

    smolscale::block_on(main_async(opts))
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Runs the main function for a node.
#[instrument(skip(opt))]
pub async fn main_async(opt: Args) -> anyhow::Result<()> {
    log::info!("themelio-core v{} initializing...", VERSION);
    let genesis = opt.genesis_config().await?;
    let netid = genesis.network;
    let storage: SharedStorage = opt.storage().await?;
    let bootstrap = opt.bootstrap().await?;
    log::info!("bootstrapping with {:?}", bootstrap);
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
