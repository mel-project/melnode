use clap::App;
use smol::*;
use std::net::{SocketAddr, TcpListener, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;
use themelio_core::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[smol_potat::main]
async fn main() {
    env_logger::Builder::from_env("THEMELIO_LOG")
        .parse_filters("themelio_core")
        .init();
    let matches = App::new("themelio-core")
        .version(VERSION)
        .author("Themelio Team")
        .arg_from_usage("--listen=[ADDR] 'Sets the address to listen to. Defaults to 0.0.0.0:0, i.e. all interfaces on a randomly selected port'")
        .arg_from_usage("--bootstrap=[BOOTSTRAP] 'A comma-separated list of bootstrapping servers.'")
        .arg_from_usage("--test-spam 'Test spamming'")
        .get_matches();
    let cfg_listen_addr = matches.value_of("listen").unwrap_or("0.0.0.0:0");
    let cfg_bootstrap: Vec<_> = matches
        .value_of("bootstrap")
        .unwrap_or("")
        .split(',')
        .filter_map(|s| s.to_socket_addrs().ok()?.next())
        .collect();
    log::info!("themelio-core v{} initializing...", VERSION);
    log::info!("bootstrapping with {:?}", cfg_bootstrap);
    let listener = Async::<TcpListener>::bind(cfg_listen_addr).unwrap();
    log::info!("initializing auditor module...");
    let auditor = Arc::new(
        Auditor::new(listener, AuditorState::new_test(), cfg_bootstrap)
            .await
            .unwrap(),
    );
    if matches.is_present("test-spam") {
        Task::spawn(test_spam_txx(auditor.clone())).detach();
    }
    loop {
        Timer::after(Duration::from_secs(30)).await;
        let state = auditor.get_netstate().await;
        log::info!("netstate is now {:#?}", state)
    }
}

async fn test_spam_txx(auditor: Arc<Auditor>) {
    let (pk, sk) = tmelcrypt::ed25519_keygen();
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
    let txx = &txx[1..];
    for tx in txx {
        Timer::after(Duration::from_millis(1000)).await;
        auditor.send_tx(tx.clone()).await;
    }
}
