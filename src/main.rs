mod args;
mod blkidx;
#[cfg(feature = "metrics")]
mod prometheus;
mod protocols;
#[cfg(feature = "metrics")]
mod public_ip_address;
mod storage;

#[cfg(feature = "metrics")]
use crate::prometheus::{AWS_INSTANCE_ID, AWS_REGION};

use std::time::Duration;
#[cfg(feature = "metrics")]
use tokio::runtime::Runtime;

use crate::protocols::{NodeProtocol, StakerProtocol};
use crate::storage::NodeStorage;

use args::Args;
use once_cell::sync::Lazy;
use structopt::StructOpt;

#[cfg(feature = "metrics")]
pub static RUNTIME: Lazy<Runtime> =
    Lazy::new(|| Runtime::new().expect("Could not create tokio runtime."));

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() -> anyhow::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "themelio_node=debug,warn");
    }
    let mut builder = env_logger::Builder::from_env("RUST_LOG");
    #[cfg(feature = "metrics")]
    {
        use std::io::Write;
        builder.format(|f, r| {
            writeln!(f, "hostname={} public_ip={} network={} region={} instance_id={} level={} message={:?}", crate::prometheus::HOSTNAME.as_str(), crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(), crate::prometheus::NETWORK.read(),
            AWS_REGION.read(),
               AWS_INSTANCE_ID.read(),
            r.level(),
            r.args())
        });
    }
    let opts = Args::from_args();

    smolscale::block_on(main_async(opts))
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Runs the main function for a node.
pub async fn main_async(opt: Args) -> anyhow::Result<()> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    log::info!("themelio-core v{} initializing...", VERSION);

    let genesis = opt.genesis_config().await?;
    let netid = genesis.network;
    let storage: NodeStorage = opt.storage().await?;
    let bootstrap = opt.bootstrap().await?;

    log::info!("bootstrapping with {:?}", bootstrap);

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
    {
        crate::prometheus::GLOBAL_STORAGE
            .set(storage)
            .ok()
            .expect("Could not write to GLOBAL_STORAGE");
        std::thread::spawn(|| RUNTIME.block_on(crate::prometheus::prometheus()));
    }

    #[cfg(feature = "dhat-heap")]
    for i in 0..300 {
        smol::Timer::after(Duration::from_secs(1)).await;
        dbg!(i);
    }

    #[cfg(not(feature = "dhat-heap"))]
    let _: u64 = smol::future::pending().await;

    Ok(())
}
