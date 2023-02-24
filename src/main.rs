use melnode::{args::MainArgs, node::Node, staker::Staker, storage::Storage};

use anyhow::Context;

use clap::Parser;
use melnet2::{wire::http::HttpBackhaul, Swarm};
use melprot::{Client, CoinChange, NodeRpcClient};
use themelio_structs::{BlockHeight, CoinID};

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() -> anyhow::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "melnode=debug,warn");
    }

    let mut builder = env_logger::Builder::from_env("RUST_LOG");

    builder.init();
    let opts = MainArgs::parse();

    smolscale::block_on(main_async(opts))
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Runs the main function for a node.
pub async fn main_async(opt: MainArgs) -> anyhow::Result<()> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    log::info!("melnode v{} initializing...", VERSION);

    let genesis = opt.genesis_config().await?;
    let netid = genesis.network;
    let storage: Storage = opt.storage().await?;
    let bootstrap = opt.bootstrap().await?;

    log::info!("bootstrapping with {:?}", bootstrap);

    let swarm: Swarm<HttpBackhaul, NodeRpcClient> =
        Swarm::new(HttpBackhaul::new(), NodeRpcClient, "themelio-node");

    // we add the bootstrap routes as "sticky" routes that never expire
    for addr in bootstrap.iter() {
        swarm.add_route(addr.to_string().into(), true).await;
    }

    let _node_prot = Node::start(
        netid,
        opt.listen_addr(),
        opt.advertise_addr(),
        storage.clone(),
        opt.index_coins,
        swarm.clone(),
    )
    .await?;

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
        let client = Client::new(netid, rpc_client);
        // Client.trust(themelio_bootstrap::checkpoint_height(netid).unwrap());
        // let snapshot = Client.snapshot().await.unwrap();
        let snapshot = client.insecure_latest_snapshot().await.unwrap();
        smolscale::spawn::<anyhow::Result<()>>(async move {
            loop {
                log::info!("*** SELF TEST STARTED! ***");
                let mut state = storage
                    .get_state(BlockHeight(9))
                    .await
                    .context("no block 1")?;
                let last_height = storage.highest_height().await.0;
                for bh in 10..=last_height {
                    let bh = BlockHeight(bh);
                    // let blk = storage.get_state(bh).await.context("no block")?.to_block();
                    let blk = storage.get_block(bh).await.context("no block")?;
                    state = state.apply_block(&blk).expect("block application failed");
                    smol::future::yield_now().await;
                    log::debug!(
                        "{}/{} replayed correctly ({:.2}%)",
                        bh,
                        last_height,
                        bh.0 as f64 / last_height as f64 * 100.0
                    );

                    // indexer test
                    if opt.index_coins {
                        if let Some(tx_0) = blk.transactions.iter().next() {
                            let recipient = tx_0.outputs[0].covhash;
                            let coin_changes = snapshot
                                .get_raw()
                                .get_coin_changes(snapshot.current_header().height, recipient)
                                .await
                                .unwrap()
                                .unwrap();

                            assert!(coin_changes
                                .contains(&CoinChange::Add(CoinID::new(tx_0.hash_nosigs(), 0))));
                        } else if let Some(proposer_action) = blk.proposer_action {
                            let reward_dest = proposer_action.reward_dest;
                            let coin_changes = snapshot
                                .get_raw()
                                .get_coin_changes(snapshot.current_header().height, reward_dest)
                                .await
                                .unwrap()
                                .unwrap();

                            // todo: this assert if failing because indexer is often behind
                            // assert!(coin_changes.contains(&CoinChange::Add(CoinID::proposer_reward(bh))));
                        }
                    }
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
