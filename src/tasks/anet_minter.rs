use std::{convert::TryInto, net::SocketAddr, time::Instant};

use crate::protocols::NetClient;
use blkstructs::{CoinID, Header, Transaction};
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
    dbg!(minimum_difficulty());
    let init_state = State::load(&cfg.state_toml).unwrap();
    log::info!("read initial state: {:?}", init_state);

    let mut netclient = NetClient::new(cfg.bootstrap);

    let coin_tip = init_state.coin_id();
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
        Ok(stdcode::deserialize(&raw_bts)?)
    }

    /// Write to a file. TODO: atomically do this
    fn save(&self, fname: &str) -> anyhow::Result<()> {
        Ok(std::fs::write(fname, stdcode::serialize(self)?)?)
    }
}

/// Measures the difficulty required to take at least 1024 secs
fn minimum_difficulty() -> usize {
    let one_sec_difficulty = (1usize..)
        .find(|difficulty| {
            let start = Instant::now();
            melpow::Proof::generate(b"hello world", *difficulty);
            // mint_on(CoinID::zero_zero(), HashVal::default(), *difficulty);
            if start.elapsed().as_millis() > 1000 {
                let speed = 2.0f64.powi(*difficulty as _) / start.elapsed().as_secs_f64();
                log::info!("speed: {} H/s", speed);
                true
            } else {
                false
            }
        })
        .unwrap();
    log::info!("one_sec_difficulty = {}", one_sec_difficulty);
    one_sec_difficulty + 10
}

/// Mint on top of an existing coin
fn mint_on(coin: CoinID, coin_height: u64, height_hash: HashVal, difficulty: usize) -> Solution {
    let chi = tmelcrypt::hash_keyed(&height_hash, &stdcode::serialize(&coin).unwrap());
    let start = Instant::now();
    let proof = melpow::Proof::generate(&chi, difficulty);
    Solution {
        coin,
        coin_height,
        difficulty,
        proof,
    }
}

struct Solution {
    coin: CoinID,
    coin_height: u64,
    difficulty: usize,
    proof: melpow::Proof,
}

impl Solution {
    /// Converts the solution to a transaction. The result is guaranteed to be valid only within the next 10 blocks!
    pub fn into_tx(self, last_header: Header) -> Transaction {
        assert!(last_header.height > self.coin_height);
        unimplemented!()
    }
}
