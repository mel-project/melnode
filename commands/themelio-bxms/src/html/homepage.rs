use crate::to_badgateway;
use askama::Template;
use blkstructs::{CoinID, Header, DENOM_DOSC, DENOM_TMEL, DENOM_TSYM};
use nodeprot::ValClient;
use num_traits::ToPrimitive;
use tide::Body;

use super::MicroUnit;

#[derive(Template)]
#[template(path = "homepage.html")]
struct HomepageTemplate {
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
pub async fn get_homepage(req: tide::Request<ValClient>) -> tide::Result<Body> {
    let last_snap = req.state().snapshot().await.map_err(to_badgateway)?;
    let mut blocks = Vec::new();
    let mut transactions = Vec::new();
    for height in (0u64..=last_snap.current_header().height).rev().take(100) {
        let old_snap = last_snap.get_older(height).await.map_err(to_badgateway)?;
        let reward_coin = old_snap
            .get_coin(CoinID::proposer_reward(height))
            .await
            .map_err(to_badgateway)?;
        let reward_amount = reward_coin.map(|v| v.coin_data.value).unwrap_or_default();
        let old_block = old_snap.current_block().await?;
        blocks.push(BlockSummary {
            header: old_block.header,
            total_weight: old_block.transactions.iter().map(|v| v.weight(0)).sum(),
            reward_amount: MicroUnit(reward_amount, "mel".into()),
        });
        // push transactions
        if transactions.len() < 100 {
            for transaction in old_block.transactions {
                transactions.push(TransactionSummary {
                    hash: hex::encode(&transaction.hash_nosigs()),
                    shorthash: hex::encode(&transaction.hash_nosigs()[0..5]),
                    height,
                    _weight: transaction.weight(0),
                    mel_moved: MicroUnit(
                        transaction
                            .outputs
                            .iter()
                            .map(|v| if v.denom == DENOM_TMEL { v.value } else { 0 })
                            .sum(),
                        "mel".into(),
                    ),
                })
            }
        }
    }

    let mel_per_dosc = (last_snap
        .get_pool(&DENOM_DOSC)
        .await
        .map_err(to_badgateway)?
        .unwrap()
        .implied_price()
        * blkstructs::dosc_inflator(last_snap.current_header().height))
    .to_f64()
    .unwrap_or_default();

    let pool = PoolSummary {
        mel_per_sym: dbg!(last_snap
            .get_pool(&DENOM_TSYM)
            .await
            .map_err(to_badgateway)?)
        .unwrap()
        .implied_price()
        .to_f64()
        .unwrap_or_default(),
        mel_per_dosc,
    };

    let mut body: Body = HomepageTemplate {
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
