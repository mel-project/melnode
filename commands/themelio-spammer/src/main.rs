use std::net::SocketAddr;

use blkstructs::{Transaction, TxKind};
use nodeprot::NodeClient;
use rand::prelude::*;
use structopt::StructOpt;
#[derive(StructOpt)]
pub struct Args {
    #[structopt(long)]
    /// A full node to connect to
    connect: SocketAddr,
}

fn main() -> anyhow::Result<()> {
    smol::future::block_on(smol::spawn(main_async()))
}

async fn main_async() -> anyhow::Result<()> {
    let args = Args::from_args();
    let client = NodeClient::new(blkstructs::NetID::Testnet, args.connect);
    for iter in 0u64.. {
        let mut buf = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut buf);
        client
            .send_tx(Transaction {
                kind: TxKind::Faucet,
                inputs: vec![],
                outputs: vec![],
                fee: 100000000,
                data: buf,
                scripts: vec![],
                sigs: vec![],
            })
            .await?;
        eprintln!("spammed {} transactions!", iter)
    }
    unreachable!()
}
