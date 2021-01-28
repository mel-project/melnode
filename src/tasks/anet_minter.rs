use std::{convert::TryInto, net::SocketAddr, time::Instant};

use crate::protocols::NetClient;
use blkstructs::{CoinData, CoinDataHeight, CoinID, Transaction};
use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use tmelcrypt::HashVal;

#[derive(Debug, StructOpt)]
pub struct AnetMinterConfig {
    /// Address for bootstrapping into the network
    #[structopt(long, default_value = "94.237.109.44:11814")]
    bootstrap: SocketAddr,

    /// Where to store the state.
    ///
    /// This should be a TOML file with three fields:
    /// - `seed_txhash`: transaction hash of the coin that the minter will build upon
    /// - `seed_index`: index of the coin that the minter will build upon
    /// - `output_addr`: address where DOSCs will be sent
    #[structopt(long)]
    state_toml: String,
}

pub async fn run_anet_minter(cfg: AnetMinterConfig) {
    let init_state = State::load(&cfg.state_toml).unwrap();
    log::info!("read initial state: {:?}", init_state);

    let mut netclient = NetClient::new(cfg.bootstrap);

    let mut coin_tip = init_state.coin_id();
    loop {
        let (latest_header, _) = netclient.last_header().await.unwrap();
        let coin = netclient
            .get_coin(latest_header, coin_tip)
            .await
            .unwrap()
            .0
            .expect("coin not found");
    }
}

/// State
#[derive(Debug, Clone, Serialize, Deserialize)]
struct State {
    seed_txhash: String,
    seed_index: u8,
    output_addr: String,
}

impl State {
    /// Gets CoinID
    fn coin_id(&self) -> CoinID {
        CoinID {
            txhash: HashVal(hex::decode(&self.seed_txhash).unwrap().try_into().unwrap()),
            index: self.seed_index,
        }
    }

    /// Read from file
    fn load(fname: &str) -> anyhow::Result<Self> {
        let raw_bts = std::fs::read(fname)?;
        Ok(bincode::deserialize(&raw_bts)?)
    }

    /// Write to a file. TODO: atomically do this
    fn save(&self, fname: &str) -> anyhow::Result<()> {
        Ok(std::fs::write(fname, bincode::serialize(self)?)?)
    }
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
pub fn mint_on(coin: CoinID, height_entropy: HashVal, difficulty: usize) -> Transaction {
    let chi = tmelcrypt::hash_keyed(&height_entropy, &bincode::serialize(&coin).unwrap());
    let proof = melpow::Proof::generate(&chi, difficulty);
    // we assume that the coin is a zero-valued mel coin
    // let txx =
    unimplemented!()
}
