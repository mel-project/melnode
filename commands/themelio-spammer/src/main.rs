use std::{
    net::SocketAddr,
    num::NonZeroU32,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::Context;
use blkstructs::{
    melvm::Covenant, CoinData, CoinID, Transaction, TxKind, DENOM_TMEL, MICRO_CONVERTER,
};
use governor::{
    clock::QuantaClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use nodeprot::NodeClient;
use rand::prelude::*;
use smol::prelude::*;
use smol_timeout::TimeoutExt;
use structopt::StructOpt;

#[derive(StructOpt)]
pub struct Args {
    #[structopt(long)]
    /// A full node to connect to
    connect: SocketAddr,

    #[structopt(long, default_value = "1")]
    /// How many transactions to send every second. Defaults to 1.
    tps: u32,
}

fn main() -> anyhow::Result<()> {
    smol::future::block_on(smol::spawn(main_async()))
}

async fn main_async() -> anyhow::Result<()> {
    let args = Args::from_args();
    let lim = Arc::new(RateLimiter::direct(
        Quota::per_second(NonZeroU32::new(args.tps).unwrap())
            .allow_burst(NonZeroU32::new(1).unwrap()),
    ));
    for _ in 0..100 {
        let client = NodeClient::new(blkstructs::NetID::Testnet, args.connect);
        let lim = lim.clone();
        smol::spawn(async move { spammer(&client, lim).await }).detach();
    }
    smol::future::pending().await
}

async fn spammer(
    client: &NodeClient,
    lim: Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock>>,
) -> anyhow::Result<()> {
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
    loop {
        static ITERS: AtomicU64 = AtomicU64::new(0);
        eprintln!(
            "spammed {} transactions!",
            ITERS.fetch_add(1, Ordering::Relaxed)
        );
        lim.until_ready().await;
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
        .signed_ed25519(sk);
        client
            .send_tx(new_tx.clone())
            .timeout(Duration::from_secs(10))
            .await
            .context("couldn't send subsequent transaction")??;
        last_coinid = CoinID {
            txhash: new_tx.hash_nosigs(),
            index: 0,
        };
        last_value -= MICRO_CONVERTER;
    }
}
