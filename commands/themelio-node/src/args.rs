use std::{net::SocketAddr, path::PathBuf};

use anyhow::Context;
use structopt::StructOpt;
use themelio_stf::{melvm::Address, BlockHeight, GenesisConfig};
use tmelcrypt::Ed25519SK;

use crate::storage::{NodeStorage, SharedStorage};

#[derive(Debug, StructOpt)]
pub struct Args {
    /// Listen address
    #[structopt(long, default_value = "0.0.0.0:11814")]
    listen: SocketAddr,

    /// Advertise address. Put your public IP address here.
    #[structopt(long)]
    advertise: Option<SocketAddr>,

    /// Bootstrap addresses. May be given as a DNS name.
    #[structopt(long, default_value = "mainnet-bootstrap.themelio.org:11814")]
    bootstrap: Vec<String>,

    /// Database path
    #[structopt(long, default_value = "/var/themelio-node/main.sqlite3")]
    database: String,

    /// Specifies the secret key for staking.
    #[structopt(long)]
    staker_sk: Option<Ed25519SK>,

    /// Bootstrap addresses for the staker network.
    #[structopt(long)]
    staker_bootstrap: Vec<SocketAddr>,

    /// Listen address for the staker network.
    #[structopt(long)]
    staker_listen: Option<SocketAddr>,

    /// Payout address for staker rewards.
    #[structopt(long)]
    staker_payout_addr: Option<Address>,

    /// If given, uses this TOML file to configure the network genesis rather than following the known testnet/mainnet genesis.
    #[structopt(long)]
    override_genesis: Option<PathBuf>,

    /// If set to true, default to the testnet. Otherwise, mainnet validation rules are used.
    #[structopt(long)]
    testnet: bool,

    /// Fee multiplier to target. Default is 1000.
    #[structopt(long, default_value = "1000")]
    target_fee_multiplier: u128,

    /// Reset last block to the given height.
    #[structopt(long)]
    emergency_reset_block: Option<BlockHeight>,
}

impl Args {
    /// Gets the advertised IP.
    pub fn advertise_addr(&self) -> Option<SocketAddr> {
        self.advertise
    }

    #[cfg(not(feature = "metrics"))]
    /// Derives the genesis configuration from the arguments
    pub async fn genesis_config(&self) -> anyhow::Result<GenesisConfig> {
        if let Some(path) = &self.override_genesis {
            let genesis_toml: Vec<u8> = smol::fs::read(&path)
                .await
                .context("cannot read genesis config")?;
            Ok(toml::from_slice(&genesis_toml).context("genesis config not a valid TOML file")?)
        } else if self.testnet {
            Ok(GenesisConfig::std_testnet())
        } else {
            Ok(GenesisConfig::std_mainnet())
        }
    }

    #[cfg(feature = "metrics")]
    /// Derives the genesis configuration from the arguments
    pub async fn genesis_config(&self) -> anyhow::Result<GenesisConfig> {
        if let Some(path) = &self.override_genesis {
            let genesis_toml: Vec<u8> = smol::fs::read(&path)
                .await
                .context("cannot read genesis config")?;
            Ok(toml::from_slice(&genesis_toml).context("genesis config not a valid TOML file")?)
        } else if self.testnet {
            *crate::prometheus::NETWORK
                .write()
                .expect("Could not get a write lock on NETWORK") = "testnet";

            Ok(GenesisConfig::std_testnet())
        } else {
            Ok(GenesisConfig::std_mainnet())
        }
    }

    /// Derives a SharedStorage from the arguments
    pub async fn storage(&self) -> anyhow::Result<SharedStorage> {
        let database =
            boringdb::Database::open(&self.database).context("cannot open boringdb database")?;
        log::debug!("database opened at {}", self.database);

        let storage = NodeStorage::new(database, self.genesis_config().await?).share();

        // Reset block. This is used to roll back history in emergencies
        if let Some(height) = self.emergency_reset_block {
            let mut storage = storage.write();
            log::warn!("*** EMERGENCY RESET TO BLOCK {} ***", height);
            let history = storage.history_mut();
            while history
                .get_tips()
                .iter()
                .any(|f| f.header().height > height)
            {
                history.delete_tips();
            }
        }

        log::debug!("node storage opened");
        Ok(storage)
    }

    /// Derives a list of bootstrap addresses
    pub async fn bootstrap(&self) -> anyhow::Result<Vec<SocketAddr>> {
        let mut bootstrap = vec![];
        for name in self.bootstrap.iter() {
            let addrs = smol::net::resolve(&name)
                .await
                .context("cannot resolve DNS bootstrap")?;
            bootstrap.extend(addrs);
        }
        Ok(bootstrap)
    }

    /// Listening address
    pub fn listen_addr(&self) -> SocketAddr {
        self.listen
    }

    /// Staker secret key
    pub async fn staker_sk(
        &self,
    ) -> anyhow::Result<Option<(Ed25519SK, SocketAddr, Vec<SocketAddr>, u128, Address)>> {
        if let Some(staker_sk) = self.staker_sk {
            let staker_listen = self
                .staker_listen
                .context("staker_listen must be set if staker_sk is set")?;
            let staker_bootstrap = self.staker_bootstrap.clone();
            let staker_payout_addr = self
                .staker_payout_addr
                .context("staker_payout_addr must be set of staker_sk is set")?;
            Ok(Some((
                staker_sk,
                staker_listen,
                staker_bootstrap,
                self.target_fee_multiplier,
                staker_payout_addr,
            )))
        } else {
            Ok(None)
        }
    }
}
