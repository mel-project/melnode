use crate::storage::Storage;

use std::{net::SocketAddr, path::PathBuf};

use anyhow::Context;
use serde::Deserialize;
use structopt::StructOpt;
use tap::Tap;
use themelio_stf::GenesisConfig;
use themelio_structs::Address;
use tmelcrypt::Ed25519SK;

#[derive(Debug, StructOpt)]
/// Command-line arguments.
pub struct Args {
    /// Listen address
    #[structopt(long, default_value = "0.0.0.0:41814")]
    listen: SocketAddr,

    /// Optional listen address for nodes using the legacy melnet protocol.
    #[structopt(long)]
    legacy_listen: Option<SocketAddr>,

    /// Advertise address. Put your public IP address here.
    #[structopt(long)]
    advertise: Option<SocketAddr>,

    /// Override bootstrap addresses. May be given as a DNS name.
    #[structopt(long, default_value = "mainnet-bootstrap.themelio.org:41814")]
    bootstrap: Vec<String>,

    /// Database path
    #[structopt(long)]
    database: Option<PathBuf>,

    /// Path to a YAML staker configuration
    #[structopt(long)]
    staker_cfg: Option<PathBuf>,

    /// If given, uses this JSON file to configure the network genesis rather than following the known testnet/mainnet genesis.
    #[structopt(long)]
    override_genesis: Option<PathBuf>,

    /// If set to true, default to the testnet. Otherwise, mainnet validation rules are used.
    #[structopt(long)]
    testnet: bool,

    /// If set to true, runs a self-test by replaying the history from genesis, ensuring that everything is correct
    #[structopt(long)]
    pub self_test: bool,

    /// Create an in-memory coin index.
    #[structopt(long)]
    pub index_coins: bool,
}

/// Staker configuration, YAML-deserializable.
#[derive(Clone, Debug, Deserialize)]
pub struct StakerConfig {
    /// ed25519 secret key of the staker
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub signing_secret: Ed25519SK,
    /// Listen address for the staker.
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub listen: SocketAddr,
    /// Bootstrap address into the staker network.
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub bootstrap: SocketAddr,
    /// Payout address
    #[serde(with = "serde_with::rust::display_fromstr")]
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
            let genesis_yaml: Vec<u8> = smol::fs::read(&path)
                .await
                .context("cannot read genesis config")?;
            Ok(serde_yaml::from_slice(&genesis_yaml)
                .context("error while parsing genesis config")?)
        } else if self.testnet {
            Ok(GenesisConfig::std_testnet())
        } else {
            Ok(GenesisConfig::std_mainnet())
        }
    }

    pub async fn storage(&self) -> anyhow::Result<Storage> {
        let genesis = self.genesis_config().await?;
        let genesis_id = tmelcrypt::hash_single(stdcode::serialize(&genesis).unwrap());
        let database_default_path = dirs::home_dir().expect("no home dir?!").tap_mut(|p| {
            p.push(".themelio-node/");
            p.push(format!("{}/", hex::encode(genesis_id.0)))
        });
        let database_base_path = self.database.clone().unwrap_or(database_default_path);
        let _history_path = database_base_path
            .clone()
            .tap_mut(|path| path.push("history"));
        let _smt_path = database_base_path
            .clone()
            .tap_mut(|path| path.push("smt.db"));

        std::fs::create_dir_all(&database_base_path)?;
        let storage = Storage::open(database_base_path, genesis).await?;

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

    /// Legacy listening address
    pub fn legacy_listen_addr(&self) -> Option<SocketAddr> {
        self.legacy_listen
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
