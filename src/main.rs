mod args;

mod node;

mod staker;
mod storage;

use std::net::SocketAddr;

use crate::{node::Node, staker::Staker, storage::Storage};

use anyhow::Context;
use args::Args;

use melnet2::wire::tcp::TcpBackhaul;
use melnet2::Swarm;
use structopt::StructOpt;
use themelio_nodeprot::{NodeRpcClient, ValClient};
use themelio_structs::BlockHeight;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() -> anyhow::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "themelio_node=debug,warn");
    }

    let mut builder = env_logger::Builder::from_env("RUST_LOG");

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
        smolscale::spawn::<anyhow::Result<()>>(async move {
            loop {
                log::info!("*** SELF TEST STARTED! ***");
                let mut state = storage
                    .get_state(BlockHeight(1))
                    .await?
                    .context("no block 1")?;
                let last_height = storage.highest_height().await?.unwrap_or_default().0;
                for bh in 2..=last_height {
                    let bh = BlockHeight(bh);
                    let blk = storage.get_state(bh).await?.context("no block")?.to_block();
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

    let swarm = Swarm::new(TcpBackhaul::new(), NodeRpcClient, "themelio-node");

    // we add the bootstrap routes as "sticky" routes that never expire
    for addr in bootstrap.iter() {
        swarm.add_route(addr.to_string().into(), true).await;
    }

    let client: Option<ValClient> = match opt.index_path {
        Some(_) => {
            // create a valclient pointing to ourselves (used by the coin indexer if needed).
            let indexer_addr: SocketAddr = "localhost:420420".parse().unwrap();
            match ValClient::connect_melnet2_tcp(netid, indexer_addr).await {
                Ok(client) => Some(client),
                Err(e) => {
                    log::warn!("error while getting ValClient for coin indexer: {:?}", e);
                    None
                }
            }
        }
        None => None,
    };

    let _node_prot = Node::new(
        netid,
        opt.listen_addr(),
        opt.legacy_listen_addr(),
        opt.advertise_addr(),
        storage.clone(),
        opt.index_path.clone(),
        swarm,
        client,
    );
    let _staker_prot = opt
        .staker_cfg()
        .await?
        .map(|cfg| Staker::new(storage.clone(), cfg));

    // #[cfg(feature = "dhat-heap")]
    // for i in 0..300 {
    //     smol::Timer::after(Duration::from_secs(1)).await;
    //     dbg!(i);
    // }

    #[cfg(not(feature = "dhat-heap"))]
    let _: u64 = smol::future::pending().await;

    Ok(())
}
