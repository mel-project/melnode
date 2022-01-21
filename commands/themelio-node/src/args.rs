use std::{net::SocketAddr, path::PathBuf};

use anyhow::Context;
use smol::{io::AsyncWriteExt, stream::StreamExt};
use std::time::Duration;
use structopt::StructOpt;
use tap::Tap;
use themelio_stf::GenesisConfig;
use themelio_structs::{Address, BlockHeight};
use tmelcrypt::Ed25519SK;

use crate::storage::NodeStorage;

#[derive(Debug, StructOpt)]
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

    /// If given, uses this JSON file to configure the network genesis rather than following the known testnet/mainnet genesis.
    #[structopt(long)]
    override_genesis: Option<PathBuf>,

    /// If set to true, default to the testnet. Otherwise, mainnet validation rules are used.
    #[structopt(long)]
    testnet: bool,

    /// If set, prunes the database at start and, on average, every 24 hours.
    #[structopt(long)]
    prune: bool,

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
                *crate::prometheus::NETWORK
                    .write()
                    .expect("Could not get a write lock on NETWORK") = "testnet";
            }

            Ok(GenesisConfig::std_testnet())
        } else {
            Ok(GenesisConfig::std_mainnet())
        }
    }

    pub async fn storage(&self) -> anyhow::Result<NodeStorage> {
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

        let storage = if self.prune {
            let from_space = NodeStorage::new(smt_db, meta_db, self.genesis_config().await?);
            let temp_metadata_path = database_base_path
                .clone()
                .tap_mut(|path| path.push("new-metadata.db"));
            let temp_metadata_journal_path = database_base_path
                .clone()
                .tap_mut(|path| path.push("new-metadata.db-journal"));
            let temp_smt_path = database_base_path
                .clone()
                .tap_mut(|path| path.push("new-smt.db"));
            let meta_db = boringdb::Database::open(&temp_metadata_path)
                .context("cannot open boringdb database")?;
            let smt_db = meshanina::Mapping::open(&temp_smt_path)
                .context("cannot open meshanina database")?;
            let mut to_space = NodeStorage::new(smt_db, meta_db, self.genesis_config().await?);
            to_space.restore_pruned(from_space.backup_pruned()).await?;
            drop(to_space);
            drop(from_space);
            smol::Timer::after(Duration::from_secs(1)).await;
            std::fs::rename(&temp_smt_path, &smt_path)?;
            std::fs::rename(&temp_metadata_path, &metadata_path)?;
            let _ = std::fs::remove_file(
                &database_base_path
                    .clone()
                    .tap_mut(|path| path.push("metadata.db-journal")),
            );
            let _ = std::fs::rename(
                &temp_metadata_journal_path,
                &database_base_path
                    .clone()
                    .tap_mut(|path| path.push("metadata.db-journal")),
            );
            eprintln!("**********************");
            smol::Timer::after(Duration::from_secs(3)).await;
            let meta_db = boringdb::Database::open(&dbg!(metadata_path))
                .context("cannot open boringdb database")?;
            let smt_db = meshanina::Mapping::open(&smt_path)
                .context("cannot open meshanina database for the second time")?;
            NodeStorage::new(smt_db, meta_db, self.genesis_config().await?)
        } else {
            NodeStorage::new(smt_db, meta_db, self.genesis_config().await?)
        };

        if std::env::var("THEMELIO_DUMP_STATE").is_ok() {
            let mut lines = storage.backup_pruned();
            let mut output = smol::fs::File::create("output.dump").await?;
            while let Some(line) = lines.next().await {
                output.write(line.as_bytes()).await?;
                output.write(b"\n").await?;
            }
            output.flush().await?;
        }

        // Reset block. This is used to roll back history in emergencies
        if let Some(_height) = self.emergency_reset_block {
            todo!()
        }

        #[cfg(not(feature = "metrics"))]
        log::debug!("node storage opened");
        #[cfg(feature = "metrics")]
        log::debug!(
            "hostname={} public_ip={} network={} node storage opened",
            crate::prometheus::HOSTNAME.as_str(),
            crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
            crate::prometheus::NETWORK.read().expect("Could not get a read lock on NETWORK.")
        );
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
