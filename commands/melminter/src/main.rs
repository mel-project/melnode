use std::{
    future::Future,
    net::SocketAddr,
    time::{Duration, Instant, SystemTime},
};

use anyhow::Context;
use blkstructs::{melvm::Covenant, CoinData, CoinDataHeight, CoinID, Denom, Transaction};
use cmdopts::{CmdOpts, InitCmdOpts, MintCmdOpts};
use nodeprot::ValClientSnapshot;
use state::MintState;
use structopt::StructOpt;
use tmelcrypt::HashVal;
mod cmdopts;
mod state;
use smol::prelude::*;

fn main() -> anyhow::Result<()> {
    let log_conf = std::env::var("RUST_LOG").unwrap_or_else(|_| "melminter=debug,warn".into());
    std::env::set_var("RUST_LOG", log_conf);
    let opts = CmdOpts::from_args();
    tracing_subscriber::fmt::init();
    smolscale::block_on(async move {
        match opts {
            CmdOpts::Init(init) => main_init(init).await,
            CmdOpts::Mint(opts) => main_mint(opts).await,
        }
    })
}

async fn main_mint(opts: MintCmdOpts) -> anyhow::Result<()> {
    let mut mint_state = MintState::read_from_file(&opts.common.persist).await?;
    let mut my_speed = compute_speed().await;
    let snap = get_snapshot(opts.common.testnet, opts.common.connect).await?;
    let max_speed = snap.current_header().dosc_speed as f64 / 30.0;
    assert_eq!(
        snap.get_coin(mint_state.chain_tip_id).await?.unwrap(),
        mint_state.chain_tip_cdh
    );

    loop {
        log::info!("** My speed: {:.3} kH/s", my_speed / 1000.0);
        log::info!("** Max speed: {:.3} kH/s", max_speed / 1000.0);
        log::info!(
            "** Estimated return: {:.2} rDOSC/day",
            my_speed * max_speed / max_speed.powi(2)
        );
        let my_difficulty = (my_speed * 3000.0).log2().ceil() as usize;
        let approx_iter = Duration::from_secs_f64(2.0f64.powi(my_difficulty as _) / my_speed);
        log::info!(
            "** Selected difficulty: {} (approx. {:?} / tx)",
            my_difficulty,
            approx_iter
        );
        let start = Instant::now();
        let deadline =
            SystemTime::now() + Duration::from_secs_f64(2.0f64.powi(my_difficulty as _) / my_speed);
        let mut tx: Transaction = mint_state
            .mint_transaction(my_difficulty)
            .or(async move {
                loop {
                    let now = SystemTime::now();
                    if let Ok(dur) = deadline.duration_since(now) {
                        log::debug!("approx {:?} left in iteration", dur);
                    }
                    smol::Timer::after(Duration::from_secs(60)).await;
                }
            })
            .await;
        let snap = repeat_fallible(|| async {
            get_snapshot(opts.common.testnet, opts.common.connect).await
        })
        .await;
        let reward_speed = 2u128.pow(my_difficulty as u32)
            / (snap.current_header().height + 5 - mint_state.chain_tip_cdh.height) as u128;
        let reward = blkstructs::calculate_reward(
            reward_speed,
            snap.current_header().dosc_speed,
            my_difficulty as u32,
        );
        let reward_nom = blkstructs::dosc_inflate_r2n(snap.current_header().height, reward);
        tx.outputs.push(CoinData {
            denom: Denom::NomDosc,
            value: reward_nom,
            additional_data: vec![],
            covhash: mint_state.payout_covhash,
        });
        tx.scripts
            .push(Covenant::std_ed25519_pk_new(opts.secret_key.to_public()));
        tx = tx
            .applied_fee(snap.current_header().fee_multiplier, 100, 0)
            .unwrap()
            .signed_ed25519(opts.secret_key);
        // broadcast and wait
        let (coin_id, cdh, hash): (CoinID, CoinDataHeight, HashVal) = repeat_fallible(|| async {
            loop {
                let snap = get_snapshot(opts.common.testnet, opts.common.connect).await?;
                let cdh = snap.get_coin(tx.get_coinid(0)).await?;
                if let Some(cdh) = cdh {
                    log::info!(
                        "***** MINTED {} µNomDOSC => {} @ {} / {} µMEL left in chain *****",
                        tx.outputs[1].value,
                        tx.outputs[1].covhash.to_addr(),
                        tx.get_coinid(1),
                        tx.outputs[0].value,
                    );
                    return Ok::<_, anyhow::Error>((
                        tx.get_coinid(0),
                        cdh.clone(),
                        snap.get_history(cdh.height)
                            .await?
                            .unwrap_or_else(|| snap.current_header())
                            .hash(),
                    ));
                } else {
                    if let Err(err) = snap.get_raw().send_tx(tx.clone()).await {
                        log::debug!("error while transmit: {:?}", err);
                    }
                    smol::Timer::after(Duration::from_secs(30)).await;
                }
            }
        })
        .await;
        mint_state.chain_tip_id = coin_id;
        mint_state.chain_tip_cdh = cdh;
        mint_state.chain_tip_hash = hash;
        my_speed = 2.0f64.powi(my_difficulty as _) / start.elapsed().as_secs_f64();
        mint_state.write_to_file(&opts.common.persist).await?;
    }
}

// Repeats something until it stops failing
async fn repeat_fallible<T, E: std::fmt::Debug, F: Future<Output = Result<T, E>>>(
    mut clos: impl FnMut() -> F,
) -> T {
    loop {
        match clos().await {
            Ok(val) => return val,
            Err(err) => log::debug!("retrying failed: {:?}", err),
        }
    }
}

// Computes difficulty
async fn compute_speed() -> f64 {
    for difficulty in 1.. {
        let start = Instant::now();
        smol::unblock(move || melpow::Proof::generate(&[], difficulty)).await;
        let elapsed = start.elapsed();
        let speed = 2.0f64.powi(difficulty as _) / elapsed.as_secs_f64();
        if elapsed.as_secs_f64() > 10.0 {
            return speed;
        }
    }
    unreachable!()
}

async fn main_init(init: InitCmdOpts) -> anyhow::Result<()> {
    log::info!("Initial CoinID: {}", init.coinid);
    log::info!(
        "Payout covhash: {}",
        HashVal::from_addr(&init.payout_addr).context("could not parse address")?
    );
    log::info!("Obtaining CoinDataHeight...");
    let snapshot = get_snapshot(init.common.testnet, init.common.connect)
        .await
        .context("cannot get snapshot")?;
    let cdh = snapshot
        .get_coin(init.coinid)
        .await
        .context("cannot get CDH from network")?;
    if let Some(cdh) = cdh {
        let mint_state = MintState {
            chain_tip_id: init.coinid,
            chain_tip_cdh: cdh.clone(),
            chain_tip_hash: snapshot.get_history(cdh.height).await?.unwrap().hash(),
            payout_covhash: HashVal::from_addr(&init.payout_addr).unwrap(),
        };
        mint_state
            .write_to_file(&init.common.persist)
            .await
            .context("cannot save persist file")?;
        log::info!("Saved to disk! {:?}", init.common.persist);
        Ok(())
    } else {
        anyhow::bail!("Did not found CoinDataHeight!")
    }
}

async fn get_snapshot(testnet: bool, connect: SocketAddr) -> anyhow::Result<ValClientSnapshot> {
    let client = nodeprot::ValClient::new(
        if testnet {
            blkstructs::NetID::Testnet
        } else {
            blkstructs::NetID::Mainnet
        },
        connect,
    );
    if testnet {
        client.trust(
            2550,
            "2b2133e34779c4043278a5d084671a7a801022605dba2721e2d164d9c1096c13"
                .parse()
                .unwrap(),
        );
    } else {
        client.trust(
            14146,
            "50f5a41c6e996d36bc05b1272a59c8adb3fe3f98de70965abd2eed0c115d2108"
                .parse()
                .unwrap(),
        );
    }
    Ok(client.snapshot().await?)
}
