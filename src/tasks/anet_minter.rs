use std::{net::SocketAddr, time::Instant};

use blkstructs::CoinID;
use structopt::StructOpt;
use tmelcrypt::HashVal;

use crate::protocols::NetClient;

#[derive(Debug, StructOpt)]
pub struct AnetMinterConfig {
    /// Address for bootstrapping into the network
    #[structopt(long, default_value = "94.237.109.44:11814")]
    bootstrap: SocketAddr,

    /// Where to send the produced DOSCS
    #[structopt(long, default_value = "./sql.db")]
    storage_path: String,
}

pub async fn run_anet_minter(cfg: AnetMinterConfig) {
    let mut netclient = NetClient::new(cfg.bootstrap);
    let (latest_header, _) = netclient.last_header().await.unwrap();
    dbg!(latest_header);
    let old_head = netclient.old_header(latest_header, 100).await.unwrap();
    dbg!(old_head);
}

/// Measures the difficulty required to take at least 1024 secs
pub fn minimum_difficulty() -> usize {
    let one_sec_difficulty = (1..)
        .find(|difficulty| {
            let start = Instant::now();
            mint_on(CoinID::zero_zero(), HashVal::default(), *difficulty);
            start.elapsed().as_millis() > 1000
        })
        .unwrap();
    log::info!("one_sec_difficulty = {}", one_sec_difficulty);
    one_sec_difficulty + 10
}

/// Mint on top of an existing coin
pub fn mint_on(coin: CoinID, height_entropy: HashVal, difficulty: usize) -> melpow::Proof {
    let chi = tmelcrypt::hash_keyed(&height_entropy, &bincode::serialize(&coin).unwrap());
    melpow::Proof::generate(&chi, difficulty)
}
