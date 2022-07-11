use crate::storage::Storage;

use std::{net::SocketAddr, path::PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use structopt::StructOpt;
use tap::Tap;
use themelio_stf::GenesisConfig;
use themelio_structs::{Address, BlockHeight};
use tmelcrypt::Ed25519SK;

#[derive(Debug, StructOpt)]
/// Command-line arguments.
pub struct Args {
    /// Listen address
    #[structopt(long, default_value = "0.0.0.0:11814")]
    listen: SocketAddr,

    /// Advertise address. Put your public IP address here.
    #[structopt(long)]
    advertise: Option<SocketAddr>,

    /// Override bootstrap addresses. May be given as a DNS name.
    #[structopt(long, default_value = "mainnet-bootstrap.themelio.org:11814")]
    bootstrap: Vec<String>,

    /// Database path
    #[structopt(long, default_value = "/var/themelio-node/")]
    database: String,

    /// Path to a YAML staker configuration
    #[structopt(long)]
    staker_cfg: Option<PathBuf>,

    /// If given, uses this JSON file to configure the network genesis rather than following the known testnet/mainnet genesis.
    #[structopt(long)]
    override_genesis: Option<PathBuf>,

    /// If set to true, default to the testnet. Otherwise, mainnet validation rules are used.
    #[structopt(long)]
    testnet: bool,

    /// Create an in-memory coin index.
    #[structopt(long)]
    pub index_coins: bool,
}

/// Staker configuration, YAML-serializable.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde_as]
pub struct StakerConfig {
    /// ed25519 secret key of the staker
    #[serde_as(as = "DisplayFromStr")]
    pub signing_secret: Ed25519SK,
    /// Listen address for the staker.
    #[serde_as(as = "DisplayFromStr")]
    pub listen: SocketAddr,
    /// Bootstrap address into the staker network.
    #[serde_as(as = "DisplayFromStr")]
    pub bootstrap: SocketAddr,
    /// Payout address
    #[serde_as(as = "DisplayFromStr")]
    pub payout_addr: Address,
    /// Target fee multiplier
    pub target_fee_multiplier: u128,
}

impl Args {
    /// Gets the advertised IP.
    pub fn advertise_addr(&self) -> Option<SocketAddr> {
        self.advertise
    }

    /// Derives the genesis configuration from the arguments
    pub async fn genesis_config(&self) -> anyhow::Result<GenesisConfig> {
        if let Some(path) = &self.override_genesis {
            let genesis_json: Vec<u8> = smol::fs::read(&path)
                .await
                .context("cannot read genesis config")?;
            Ok(serde_json::from_slice(&genesis_json)
                .context("genesis config not a valid TOML file")?)
        } else if self.testnet {
            #[cfg(feature = "metrics")]
            {
                *crate::prometheus::NETWORK.write() = "testnet";
            }

            Ok(GenesisConfig::std_testnet())
        } else {
            Ok(GenesisConfig::std_mainnet())
        }
    }

    pub async fn storage(&self) -> anyhow::Result<Storage> {
        let database_base_path = PathBuf::from(self.database.to_string());
        let metadata_path = database_base_path
            .clone()
            .tap_mut(|path| path.push("metadata.db"));
        let smt_path = database_base_path
            .clone()
            .tap_mut(|path| path.push("smt.db"));

        std::fs::create_dir_all(&database_base_path)?;
        let meta_db =
            boringdb::Database::open(&metadata_path).context("cannot open boringdb database")?;
        let smt_db =
            meshanina::Mapping::open(&smt_path).context("cannot open meshanina database")?;
        log::debug!("database opened at {}", self.database);

        let storage = Storage::new(smt_db, meta_db, self.genesis_config().await?);

        log::debug!("node storage opened");

        Ok(storage)
    }

    /// Derives a list of bootstrap addresses
    pub async fn bootstrap(&self) -> anyhow::Result<Vec<SocketAddr>> {
        if !self.bootstrap.is_empty() {
            let mut bootstrap = vec![];
            for name in self.bootstrap.iter() {
                let addrs = smol::net::resolve(&name)
                    .await
                    .context("cannot resolve DNS bootstrap")?;
                bootstrap.extend(addrs);
            }
            Ok(bootstrap)
        } else {
            Ok(themelio_bootstrap::bootstrap_routes(
                self.genesis_config().await?.network,
            ))
        }
    }

    /// Listening address
    pub fn listen_addr(&self) -> SocketAddr {
        self.listen
    }

    /// Staker secret key
    pub async fn staker_cfg(&self) -> anyhow::Result<Option<StakerConfig>> {
        if let Some(path) = self.staker_cfg.as_ref() {
            let s = std::fs::read_to_string(path)?;
            let lele: StakerConfig = serde_yaml::from_str(&s)?;
            Ok(Some(lele))
        } else {
            Ok(None)
        }
    }
}
