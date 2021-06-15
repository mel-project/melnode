use crate::to_badgateway;
use askama::Template;
use futures_util::stream::FuturesOrdered;
use futures_util::StreamExt;
use num_traits::ToPrimitive;
use themelio_nodeprot::ValClient;
use themelio_stf::{CoinID, Denom, Header, NetID};
use tide::Body;

use super::{MicroUnit, RenderTimeTracer};

#[derive(Template)]
#[template(path = "homepage.html")]
struct HomepageTemplate {
    testnet: bool,
    blocks: Vec<BlockSummary>,
    transactions: Vec<TransactionSummary>,
    pool: PoolSummary,
}

// A block summary for the homepage.
struct BlockSummary {
    header: Header,
    total_weight: u128,
    reward_amount: MicroUnit,
}

// A transaction summary for the homepage.
struct TransactionSummary {
    hash: String,
    shorthash: String,
    height: u64,
    _weight: u128,
    mel_moved: MicroUnit,
}

// A pool summary for the homepage.
struct PoolSummary {
    mel_per_sym: f64,
    mel_per_dosc: f64,
}

/// Homepage
#[tracing::instrument(skip(req))]
pub async fn get_homepage(req: tide::Request<ValClient>) -> tide::Result<Body> {
    let _render = RenderTimeTracer::new("homepage");

    let last_snap = req.state().snapshot().await.map_err(to_badgateway)?;
    let mut blocks = Vec::new();
    let mut transactions = Vec::new();

    let mut futs = FuturesOrdered::new();
    for height in (0u64..=last_snap.current_header().height).rev().take(30) {
        let last_snap = last_snap.clone();
        futs.push(async move {
            log::debug!("rendering block {}", height);
            let old_snap = last_snap.get_older(height).await.map_err(to_badgateway)?;
            let reward_coin = old_snap
                .get_coin(CoinID::proposer_reward(height))
                .await
                .map_err(to_badgateway)?;
            let reward_amount = reward_coin.map(|v| v.coin_data.value).unwrap_or_default();
            let old_block = old_snap.current_block().await?;
            Ok::<_, tide::Error>((old_block, reward_amount))
        });
    }

    while let Some(inner) = futs.next().await {
        let (block, reward) = inner?;
        blocks.push(BlockSummary {
            header: block.header,
            total_weight: block.transactions.iter().map(|v| v.weight()).sum(),
            reward_amount: MicroUnit(reward, "MEL".into()),
        });
        // push transactions
        if transactions.len() < 30 {
            for transaction in block.transactions {
                if transactions.len() < 30 {
                    transactions.push(TransactionSummary {
                        hash: hex::encode(&transaction.hash_nosigs().0),
                        shorthash: hex::encode(&transaction.hash_nosigs().0[0..5]),
                        height: block.header.height,
                        _weight: transaction.weight(),
                        mel_moved: MicroUnit(
                            transaction
                                .outputs
                                .iter()
                                .map(|v| if v.denom == Denom::Mel { v.value } else { 0 })
                                .sum::<u128>()
                                + transaction.fee,
                            "MEL".into(),
                        ),
                    })
                }
            }
        }
    }

    let mel_per_dosc = (last_snap
        .get_pool(Denom::NomDosc)
        .await
        .map_err(to_badgateway)?
        .unwrap()
        .implied_price()
        * themelio_stf::dosc_inflator(last_snap.current_header().height))
    .to_f64()
    .unwrap_or_default();

    let pool = PoolSummary {
        mel_per_sym: last_snap
            .get_pool(Denom::Sym)
            .await
            .map_err(to_badgateway)?
            .unwrap()
            .implied_price()
            .to_f64()
            .unwrap_or_default(),
        mel_per_dosc,
    };

    let mut body: Body = HomepageTemplate {
        testnet: req.state().netid() == NetID::Testnet,
        blocks,
        transactions,
        pool,
    }
    .render()
    .unwrap()
    .into();
    body.set_mime("text/html");
    Ok(body)
}
