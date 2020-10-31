#![feature(try_blocks)]

//! [Themelio](https://themelio.org) is a work-in-progress public blockchain focused on security, performance, and long-term stability 


mod auditor;
pub use auditor::*;
use main_anet_client::{AnetClientConfig, run_anet_client};
mod stakeholder;
pub use stakeholder::*;
mod common;
use common::*;
mod storage;
use parking_lot::RwLock;
use smol::net::TcpListener;
use std::time::Duration;
use std::{sync::Arc};
use std::{
    net::{SocketAddr, ToSocketAddrs},
};
pub use storage::*;
mod client;
mod main_anet_client;
use structopt::StructOpt;
mod client_protocol;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, StructOpt)]
pub enum Config {
    /// Runs a network node (auditor/stakeholder) that serves clients and other nodes.
    Node(NodeConfig),
    /// Runs a thin client that connects to other nodes.
    AnetClient(AnetClientConfig),
}

#[derive(Debug, StructOpt)]
pub struct NodeConfig {
    /// Listen address
    #[structopt(long, default_value = "0.0.0.0:0")]
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
}

pub async fn run_main(opt: Config) {
    match opt {
        Config::Node(cfg) => run_node(cfg).await,
        Config::AnetClient(cfg) => run_anet_client(cfg).await,
    }
}

/// Runs the main function for a node.
async fn run_node(opt: NodeConfig) {
    let _ = std::fs::create_dir_all(&opt.database);

    log::info!("themelio-core v{} initializing...", VERSION);
    log::info!("bootstrapping with {:?}", opt.bootstrap);
    let listener = TcpListener::bind(opt.listen).await.unwrap();
    let storage = Arc::new(RwLock::new(Storage::open_testnet(&opt.database).unwrap()));
    let auditor = Auditor::new(listener, storage.clone(), &opt.bootstrap)
        .await
        .unwrap();
    if opt.test_spam { 
        smolscale::spawn(test_spam_txx(auditor.clone())).detach();
    } 
    if let Some(sh_no) = opt.test_stakeholder {
        smolscale::spawn(test_stakeholder(sh_no, auditor.clone(), storage.clone())).detach();
    }

    // Storage syncer
    loop {
        Timer::after(Duration::from_secs(600)).await;
        {
            let storage = storage.clone();
            smol::unblock(move || storage.write().sync()).await;
        }
    }
}

async fn test_stakeholder(sh_no: usize, auditor: Auditor, storage: Arc<RwLock<Storage>>) {
    log::info!("testnet stakeholder {}", sh_no);
    let socket_addr = "0.0.0.0:0".to_socket_addrs().unwrap().next().unwrap();
    let _actor = Stakeholder::new(
        socket_addr,
        auditor,
        storage,
        if sh_no == 0 {
            insecure_testnet_keygen(sh_no).1
        } else {
            tmelcrypt::ed25519_keygen().1
        },
    )
    .await
    .unwrap();
    // block forever now
    loop {
        Timer::after(Duration::from_secs(10000000)).await;
    }
}

async fn test_spam_txx(auditor: Auditor) {
    let (_, sk) = tmelcrypt::ed25519_keygen();
    let txx = blkstructs::testing::random_valid_txx(
        &mut rand::thread_rng(),
        blkstructs::CoinID {
            txhash: tmelcrypt::HashVal::default(),
            index: 0,
        },
        blkstructs::CoinData {
            conshash: blkstructs::melscript::Script::always_true().hash(),
            value: blkstructs::MICRO_CONVERTER * 1000,
            cointype: blkstructs::COINTYPE_TMEL.to_owned(),
        },
        sk,
        &blkstructs::melscript::Script::always_true(),
    );
    log::info!("starting spamming with {} txx", txx.len());
    //let txx = &txx[1..];
    for tx in txx {
        Timer::after(Duration::from_millis(1000)).await;
        auditor
            .send_ret(|s| AuditorMsg::SendTx(tx, s))
            .await
            .unwrap();
    }
}
