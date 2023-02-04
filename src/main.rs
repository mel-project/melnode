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

    log::info!("bootstrapping with {:?}", bootstrap);

    let swarm: Swarm<TcpBackhaul, NodeRpcClient> =
        Swarm::new(TcpBackhaul::new(), NodeRpcClient, "themelio-node");

    // we add the bootstrap routes as "sticky" routes that never expire
    for addr in bootstrap.iter() {
        swarm.add_route(addr.to_string().into(), true).await;
    }

    let _node_prot = Node::new(
        netid,
        opt.listen_addr(),
        opt.legacy_listen_addr(),
        opt.advertise_addr(),
        storage.clone(),
        opt.index_coins,
        swarm.clone(),
    )
    .await;

    let _staker_prot = opt
        .staker_cfg()
        .await?
        .map(|cfg| Staker::new(storage.clone(), cfg));

    if opt.self_test {
        let storage = storage.clone();

        let rpc_client = swarm
            .connect(opt.listen_addr().to_string().into())
            .await
            .unwrap();
        let valclient = ValClient::new(netid, rpc_client);
        let snapshot = valclient.insecure_latest_snapshot().await.unwrap();
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

                    let proposer_addr = blk.proposer_action.unwrap().reward_dest;
                    let coin_changes = snapshot
                        .get_raw()
                        .get_coin_changes(snapshot.current_header().height, proposer_addr)
                        .await
                        .unwrap();
                    println!("coin changes size: {}", coin_changes.unwrap().len());
                }
            }
        })
        .detach();
    }

    // #[cfg(feature = "dhat-heap")]
    // for i in 0..300 {
    //     smol::Timer::after(Duration::from_secs(1)).await;
    //     dbg!(i);
    // }

    #[cfg(not(feature = "dhat-heap"))]
    let _: u64 = smol::future::pending().await;

    Ok(())
}
