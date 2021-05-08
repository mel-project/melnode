use blkstructs::CoinID;
use std::{net::SocketAddr, path::PathBuf};
use structopt::StructOpt;
use tmelcrypt::Ed25519SK;
#[derive(Debug, StructOpt, Clone)]
pub enum CmdOpts {
    Init(InitCmdOpts),
    Mint(MintCmdOpts),
}

#[derive(Debug, StructOpt, Clone)]
pub struct InitCmdOpts {
    #[structopt(long)]
    /// The initial coin-id, in txhash-index. For example, "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef-123"
    pub coinid: CoinID,

    #[structopt(long)]
    /// Payout address.
    pub payout_addr: String,

    #[structopt(flatten)]
    pub common: CommonCmdOpts,
}

#[derive(Debug, StructOpt, Clone)]
pub struct MintCmdOpts {
    #[structopt(long)]
    /// The unlocking secret key, in hex.
    pub secret_key: Ed25519SK,

    #[structopt(flatten)]
    pub common: CommonCmdOpts,
}

#[derive(Debug, StructOpt, Clone)]
pub struct CommonCmdOpts {
    #[structopt(long)]
    /// Where to save the persistence JSON file.
    pub persist: PathBuf,

    #[structopt(long)]
    /// Whether or not to use the testnet.
    pub testnet: bool,

    #[structopt(long)]
    /// IP:host of the full node to connect to.
    pub connect: SocketAddr,
}
