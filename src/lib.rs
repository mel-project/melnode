//! [Themelio](https://themelio.org) is a public blockchain focused
//! on security, performance, and long-term stability.

mod config;
mod dal;
mod protocols;
mod services;
mod tasks;

use structopt::StructOpt;
use tasks::{AnetClientConfig, AnetMinterConfig, NodeConfig};

#[derive(Debug, StructOpt)]
pub enum Config {
    /// Runs a network node (auditor/stakeholder) that serves clients and other nodes.
    AnetNode(NodeConfig),
    /// Runs a thin client that connects to other nodes.
    AnetClient(AnetClientConfig),
    /// Runs a minter.
    AnetMinter(AnetMinterConfig),
}

pub async fn run_main(opt: Config) {
    match opt {
        Config::AnetNode(cfg) => tasks::run_node(cfg).await,
        Config::AnetClient(cfg) => tasks::run_anet_client(cfg).await,
        Config::AnetMinter(cfg) => tasks::run_anet_minter(cfg).await,
    }
}
