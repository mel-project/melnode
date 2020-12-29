mod dal;
mod protocols;
mod services;
mod tasks;

use structopt::StructOpt;
use tasks::{AnetClientConfig, NodeConfig};

#[derive(Debug, StructOpt)]
pub enum Config {
    /// Runs a network node (auditor/stakeholder) that serves clients and other nodes.
    Node(NodeConfig),
    /// Runs a thin client that connects to other nodes.
    AnetClient(AnetClientConfig),
}

pub async fn run_main(opt: Config) {
    match opt {
        Config::Node(cfg) => tasks::run_node(cfg).await,
        Config::AnetClient(cfg) => tasks::run_anet_client(cfg).await,
    }
}
