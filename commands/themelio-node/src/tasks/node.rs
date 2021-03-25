use std::collections::HashMap;

use crate::protocols::{NodeProtocol, StakerProtocol};
use crate::services::insecure_testnet_keygen;
use crate::{config::VERSION, services::NodeStorage};
use blkstructs::{melvm, GenesisConfig, StakeDoc};
use smol::net::SocketAddr;
use structopt::StructOpt;
use tmelcrypt::HashVal;
use tracing::instrument;
#[derive(Debug, StructOpt)]
pub struct NodeConfig {
    /// Listen address
    #[structopt(long)]
    listen: SocketAddr,

    /// Bootstrap addresses
    #[structopt(long)]
    bootstrap: Vec<SocketAddr>,

    /// Test spam
    #[structopt(long)]
    test_spam: bool,

    /// Database path
    #[structopt(long, default_value = "/tmp/testnet")]
    database: String,

    /// Testnet type
    #[structopt(long)]
    test_stakeholder: Option<usize>,

    /// Listen address for the staker network.
    #[structopt(long)]
    listen_staker: Option<SocketAddr>,
}

/// Runs the main function for a node.
#[instrument(skip(opt))]
pub async fn run_node(opt: NodeConfig) {
    let _ = std::fs::create_dir_all(&opt.database);
    log::info!("themelio-core v{} initializing...", VERSION);
    log::info!("bootstrapping with {:?}", opt.bootstrap);
    let storage =
        NodeStorage::new(sled::open(&opt.database).unwrap(), testnet_genesis_config().await).share();
    let _node_prot = NodeProtocol::new(opt.listen, opt.bootstrap.clone(), storage.clone());
    let _staker_prot = if let Some(v) = opt.test_stakeholder {
        let my_sk = insecure_testnet_keygen(v).1;
        Some(
            StakerProtocol::new(
                opt.listen_staker.unwrap(),
                opt.bootstrap.clone(),
                storage.clone(),
                my_sk,
            )
            .unwrap(),
        )
    } else {
        None
    };

    smol::future::pending::<()>().await;
}

pub async fn testnet_genesis_config() -> GenesisConfig {
    GenesisConfig {
        network: blkstructs::NetID::Testnet,
        init_micromels: 1 << 100,
        init_covhash: melvm::Covenant::always_true().hash(),
        stakes: {
            let mut toret = HashMap::new();
            toret.insert(
                HashVal::default(),
                StakeDoc {
                    pubkey: insecure_testnet_keygen(0).0,
                    e_start: 0,
                    e_post_end: 1 << 32,
                    syms_staked: 1 << 100,
                },
            );
            toret
        },
        init_fee_pool: 1 << 100,
    }
}
