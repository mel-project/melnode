use std::time::{Duration, SystemTime};

use crate::{notfound, to_badgateway, to_badreq};
use anyhow::Context;
use askama::Template;
use blkstructs::{DENOM_DOSC, DENOM_TMEL, DENOM_TSYM};
use futures_util::stream::FuturesUnordered;
use nodeprot::ValClient;
use num_traits::ToPrimitive;
use serde::Serialize;
use smol::prelude::*;

use super::RenderTimeTracer;

#[derive(Template)]
#[template(path = "pool.html")]
struct PoolTemplate {
    denom: String,
    last_week_json: String,
    last_month_json: String,
}

#[tracing::instrument(skip(req))]
pub async fn get_poolpage(req: tide::Request<ValClient>) -> tide::Result<tide::Body> {
    let _render = RenderTimeTracer::new("poolpage");
    let denom = req.param("denom").map(|v| v.to_string())?;
    let denom = hex::decode(&denom).map_err(to_badreq)?;

    let last_week = pool_items(req.state(), &denom, 20160).await?;
    let last_month = pool_items(req.state(), &denom, 86400).await?;
    let mut body: tide::Body = PoolTemplate {
        denom: match &denom {
            x if x == DENOM_TMEL => "TMEL".into(),
            x if x == DENOM_TSYM => "TSYM".into(),
            x if x == DENOM_DOSC => "nDOSC".into(),
            other => format!("Other ({})", hex::encode(&other)),
        },
        last_month_json: serde_json::to_string(&last_month).unwrap(),
        last_week_json: serde_json::to_string(&last_week).unwrap(),
    }
    .render()
    .unwrap()
    .into();
    body.set_mime("text/html");
    Ok(body)
}

async fn pool_items(
    client: &ValClient,
    denom: &[u8],
    blocks: u64,
) -> tide::Result<Vec<PoolDataItem>> {
    let snapshot = client.snapshot().await.map_err(to_badgateway)?;
    let last_height = snapshot.current_header().height;
    let blocks = last_height.min(blocks);
    const DIVIDER: u64 = 100;
    // at most DIVIDER points
    let snapshot = &snapshot;
    let mut item_futs = FuturesUnordered::new();
    for height in (last_height - blocks..=last_height).step_by((blocks / DIVIDER) as usize) {
        item_futs.push(async move {
            let old_snap = snapshot.get_older(height).await.map_err(to_badgateway)?;
            let pool_info = old_snap
                .get_pool(denom)
                .await
                .map_err(to_badgateway)?
                .ok_or_else(notfound)?;
            let price = pool_info.implied_price().to_f64().unwrap_or_default();
            let liquidity = pool_info.mels as f64 * 2.0;
            Ok::<_, tide::Error>(PoolDataItem {
                date: chrono::Utc::now()
                    .checked_sub_signed(
                        chrono::Duration::from_std(
                            Duration::from_secs(30) * (last_height - height) as u32,
                        )
                        .unwrap(),
                    )
                    .unwrap()
                    .naive_utc(),
                height,
                price,
                liquidity,
            })
        })
    }
    // Gather the stuff
    let mut output = vec![];
    while let Some(res) = item_futs.next().await {
        log::debug!("loading pooldata {}/{}", output.len(), DIVIDER);
        output.push(res?);
    }
    output.sort_unstable_by_key(|v| v.height);
    Ok(output)
}

#[derive(Serialize)]
struct PoolDataItem {
    date: chrono::NaiveDateTime,
    height: u64,
    price: f64,
    liquidity: f64,
}
