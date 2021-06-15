use std::time::Duration;

use crate::{notfound, to_badgateway, to_badreq};
use anyhow::Context;
use askama::Template;
use futures_util::stream::FuturesUnordered;
use num_traits::ToPrimitive;
use serde::Serialize;
use smol::prelude::*;
use themelio_nodeprot::ValClient;
use themelio_stf::{Denom, NetID, MICRO_CONVERTER};

use super::{friendly_denom, RenderTimeTracer};

#[derive(Template)]
#[template(path = "pool.html")]
struct PoolTemplate {
    testnet: bool,
    denom: String,
    last_day_json: String,
    last_week_json: String,
    last_month_json: String,
    all_time_json: String,
    last_item: PoolDataItem,
}

#[tracing::instrument(skip(req))]
pub async fn get_poolpage(req: tide::Request<ValClient>) -> tide::Result<tide::Body> {
    let _render = RenderTimeTracer::new("poolpage");
    let denom = req.param("denom").map(|v| v.to_string())?;
    let denom = Denom::from_bytes(&hex::decode(&denom).map_err(to_badreq)?)
        .ok_or_else(|| to_badreq(anyhow::anyhow!("bad")))?;
    let snapshot = req.state().snapshot().await.map_err(to_badgateway)?;
    let last_height = snapshot.current_header().height;
    let last_day = pool_items(req.state(), denom, 2880).await?;
    let last_week = pool_items(req.state(), denom, 20160).await?;
    let last_month = pool_items(req.state(), denom, 86400).await?;
    let all_time = pool_items(req.state(), denom, last_height).await?;
    let mut body: tide::Body = PoolTemplate {
        testnet: req.state().netid() == NetID::Testnet,
        denom: friendly_denom(denom),
        last_day_json: serde_json::to_string(&last_day).unwrap(),
        last_month_json: serde_json::to_string(&last_month).unwrap(),
        last_week_json: serde_json::to_string(&last_week).unwrap(),
        all_time_json: serde_json::to_string(&all_time).unwrap(),
        last_item: last_week.last().context("no last")?.clone(),
    }
    .render()
    .unwrap()
    .into();
    body.set_mime("text/html");
    Ok(body)
}

async fn pool_items(
    client: &ValClient,
    denom: Denom,
    blocks: u64,
) -> tide::Result<Vec<PoolDataItem>> {
    let snapshot = client.snapshot().await.map_err(to_badgateway)?;
    let last_height = snapshot.current_header().height;
    let blocks = last_height.min(blocks);
    const DIVIDER: u64 = 300;
    // at most DIVIDER points
    let snapshot = &snapshot;
    let mut item_futs = FuturesUnordered::new();
    for height in (last_height - blocks..=last_height)
        .rev()
        .step_by((blocks / DIVIDER) as usize)
    {
        item_futs.push(async move {
            let old_snap = snapshot.get_older(height).await.map_err(to_badgateway)?;
            let pool_info = old_snap
                .get_pool(denom)
                .await
                .map_err(to_badgateway)?
                .ok_or_else(notfound)?;
            let price = pool_info.implied_price().to_f64().unwrap_or_default();
            let liquidity = pool_info.mels as f64 * 2.0 / MICRO_CONVERTER as f64;
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

#[derive(Serialize, Clone)]
struct PoolDataItem {
    date: chrono::NaiveDateTime,
    height: u64,
    price: f64,
    liquidity: f64,
}
