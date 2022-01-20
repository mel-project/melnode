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
use governor::{
    clock::QuantaClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use priority_queue::PriorityQueue;
use rand::prelude::*;
use smol_timeout::TimeoutExt;
use structopt::StructOpt;
use themelio_nodeprot::NodeClient;
use themelio_stf::melvm::Covenant;
use themelio_structs::{CoinData, CoinID, Denom, NetID, Transaction, TxKind, MICRO_CONVERTER};
use tmelcrypt::Ed25519SK;

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
    for wkr in 0..args.tps.max(100) {
        let client = NodeClient::new(NetID::Testnet, args.connect);
        let lim = lim.clone();
        smol::spawn(async move {
            loop {
                let lim = lim.clone();
                if let Err(err) = spammer(&client, lim).await {
                    eprintln!("restarting worker {}: {:?}", wkr, err)
                }
            }
        })
        .detach();
    }
    smol::future::pending().await
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct QueueEntry {
    coinid: CoinID,
    value: u128,
    unlock_key: Ed25519SK,
}

async fn spammer(
    client: &NodeClient,
    lim: Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock>>,
) -> anyhow::Result<()> {
    let first_queue_entry = {
        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let my_covenant = Covenant::std_ed25519_pk_new(pk);
        let mut buf = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut buf);
        let first_tx = Transaction {
            kind: TxKind::Faucet,
            inputs: vec![],
            outputs: vec![CoinData {
                denom: Denom::Mel,
                value: (1 << 40).into(),
                additional_data: vec![],
                covhash: my_covenant.hash(),
            }],
            fee: MICRO_CONVERTER.into(),
            data: buf,
            covenants: vec![],
            sigs: vec![],
        };

        client
            .send_tx(first_tx.clone())
            .await
            .context("cannot send first tx")?;
        QueueEntry {
            coinid: CoinID {
                txhash: first_tx.hash_nosigs(),
                index: 0,
            },
            value: 1 << 40,
            unlock_key: sk,
        }
    };
    let mut coin_queue = PriorityQueue::new();
    coin_queue.push(first_queue_entry, rand::random::<u64>());
    loop {
        static ITERS: AtomicU64 = AtomicU64::new(0);
        eprintln!(
            "spammed {} transactions!",
            ITERS.fetch_add(1, Ordering::Relaxed)
        );
        lim.until_ready().await;

        let num_to_gather = (rand::random::<usize>() % 5).max(1).min(coin_queue.len());
        let inputs = (0..num_to_gather)
            .map(|_| coin_queue.pop().unwrap().0)
            .collect::<Vec<_>>();
        let num_outputs = (rand::random::<usize>() % 5).max(1);
        let total_input = inputs.iter().map(|v| v.value).sum::<u128>();
        let outputs = (0..num_outputs)
            .map(|_| (total_input - MICRO_CONVERTER) / (num_outputs as u128))
            .map(|value| {
                let (pk, sk) = tmelcrypt::ed25519_keygen();
                (
                    CoinData {
                        value: value.into(),
                        denom: Denom::Mel,
                        covhash: Covenant::std_ed25519_pk_new(pk).hash(),
                        additional_data: vec![],
                    },
                    sk,
                )
            })
            .collect::<Vec<_>>();
        let fee: u128 = total_input - outputs.iter().map(|v| u128::from(v.0.value)).sum::<u128>();

        let mut new_tx = Transaction {
            kind: TxKind::Normal,
            inputs: inputs.iter().map(|v| v.coinid).collect(),
            outputs: outputs.iter().map(|v| v.0.clone()).collect(),
            fee: fee.into(),
            covenants: inputs
                .iter()
                .map(|v| Covenant::std_ed25519_pk_new(v.unlock_key.to_public()).0)
                .collect(),
            sigs: vec![],
            data: vec![],
        };
        for sk in inputs.iter().map(|v| v.unlock_key) {
            new_tx = new_tx.signed_ed25519(sk);
        }
        for (index, (output, unlock_key)) in outputs.iter().enumerate() {
            let entry = QueueEntry {
                coinid: CoinID {
                    txhash: new_tx.hash_nosigs(),
                    index: index as u8,
                },
                value: output.value.into(),
                unlock_key: *unlock_key,
            };
            coin_queue.push(entry, rand::random::<u64>());
        }
        client
            .send_tx(new_tx.clone())
            .timeout(Duration::from_secs(10))
            .await
            .context("couldn't send subsequent transaction")??;
    }
}
