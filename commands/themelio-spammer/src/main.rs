use std::net::SocketAddr;

use anyhow::Context;
use blkstructs::{
    melvm::Covenant, CoinData, CoinID, Transaction, TxKind, DENOM_TMEL, MICRO_CONVERTER,
};
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

    let (pk, sk) = tmelcrypt::ed25519_keygen();
    let my_covenant = Covenant::std_ed25519_pk(pk);
    let first_tx = {
        let mut buf = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut buf);
        Transaction {
            kind: TxKind::Faucet,
            inputs: vec![],
            outputs: vec![CoinData {
                denom: DENOM_TMEL.into(),
                value: 1 << 32,
                additional_data: vec![],
                covhash: my_covenant.hash(),
            }],
            fee: 1087000000,
            data: buf,
            scripts: vec![],
            sigs: vec![],
        }
    };

    client
        .send_tx(first_tx.clone())
        .await
        .context("cannot send first tx")?;

    let mut last_coinid = CoinID {
        txhash: first_tx.hash_nosigs(),
        index: 0,
    };
    let mut last_value = 1 << 32;

    for iter in 1u64.. {
        eprintln!("spammed {} transactions!", iter);
        let new_tx = Transaction {
            kind: TxKind::Normal,
            inputs: vec![last_coinid],
            outputs: vec![CoinData {
                denom: DENOM_TMEL.into(),
                value: last_value - MICRO_CONVERTER,
                additional_data: vec![],
                covhash: my_covenant.hash(),
            }],
            fee: MICRO_CONVERTER,
            scripts: vec![my_covenant.clone()],
            sigs: vec![],
            data: vec![],
        }
        .sign_ed25519(sk);
        dbg!(new_tx.weight());
        client
            .send_tx(new_tx.clone())
            .await
            .context("couldn't send subsequent transaction")?;
        last_coinid = CoinID {
            txhash: new_tx.hash_nosigs(),
            index: 0,
        };
        last_value -= MICRO_CONVERTER;
    }
    unreachable!()
}
