use std::net::SocketAddr;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct AnetMinterConfig {
    /// Address for bootstrapping into the network
    #[structopt(long, default_value = "94.237.109.44:11814")]
    bootstrap: SocketAddr,

    /// Where to send the produced DOSCS
    #[structopt(long, default_value = "./sql.db")]
    storage_path: String,
}

pub async fn run_anet_minter(cfg: AnetMinterConfig) {}
