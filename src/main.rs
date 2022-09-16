mod args;
mod blkidx;
#[cfg(feature = "metrics")]
mod prometheus;
mod protocols;
mod storage;

#[cfg(feature = "metrics")]
use crate::prometheus::{AWS_INSTANCE_ID, AWS_REGION};
#[cfg(feature = "metrics")]
use std::time::Duration;

use crate::protocols::{NodeProtocol, StakerProtocol};
use crate::storage::Storage;

use anyhow::Context;
use args::Args;

use melnet2::wire::tcp::TcpBackhaul;
use melnet2::Swarm;
use structopt::StructOpt;
use themelio_nodeprot::NodeRpcClient;
use themelio_structs::BlockHeight;

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
            writeln!(f, "hostname={} public_ip={} network={} region={} instance_id={} level={} message={:?}", crate::prometheus::HOSTNAME.as_str(), crate::prometheus::PUBLIC_IP_ADDRESS.as_str(), crate::prometheus::NETWORK.read(),
            AWS_REGION.read(),
               AWS_INSTANCE_ID.read(),
            r.level(),
            r.args())
        });
    }
    builder.init();
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
    let storage: Storage = opt.storage().await?;
    let bootstrap = opt.bootstrap().await?;

    if opt.self_test {
        let storage = storage.clone();
        smolscale::spawn(async move {
            loop {
                log::info!("*** SELF TEST STARTED! ***");
                let mut state = storage.get_state(BlockHeight(1)).expect("no block 1");
                let last_height = storage.highest_height().0;
                for bh in 2..=last_height {
                    let bh = BlockHeight(bh);
                    let blk = storage.get_state(bh).expect("no block").to_block();
                    state = state.apply_block(&blk).expect("block application failed");
                    smol::future::yield_now().await;
                    log::debug!(
                        "{}/{} replayed correctly ({:.2}%)",
                        bh,
                        last_height,
                        bh.0 as f64 / last_height as f64 * 100.0
                    );
                }
            }
        })
        .detach();
    }

    log::info!("bootstrapping with {:?}", bootstrap);

    // TODO: move this into a helper?
    let swarm = Swarm::new(TcpBackhaul::new(), NodeRpcClient, "themelio-node");
    for addr in bootstrap.iter() {
        swarm.add_route(addr.to_string().into(), false).await;
    }
    if let Some(advertise_addr) = opt.advertise_addr() {
        swarm.add_route(advertise_addr.to_string().into(), false);
    }
    let _node_prot = NodeProtocol::new(
        netid,
        opt.listen_addr(),
        opt.advertise_addr(),
        bootstrap,
        storage.clone(),
        opt.index_coins,
        swarm,
    );
    let _staker_prot = if let Some(cfg) = opt.staker_cfg().await? {
        Some(StakerProtocol::new(storage.clone(), cfg)?)
    } else {
        None
    };

    #[cfg(feature = "metrics")]
    {
        use async_compat::CompatExt;
        crate::prometheus::GLOBAL_STORAGE
            .set(storage)
            .ok()
            .expect("Could not write to GLOBAL_STORAGE");
        smolscale::spawn(crate::prometheus::prometheus().compat()).detach();
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
