use std::convert::TryInto;

use nodeprot::ValClient;
use std::fmt::Debug;
use tide::{Body, StatusCode};
use tmelcrypt::HashVal;

fn to_badreq<E: Into<anyhow::Error> + Send + 'static + Sync + Debug>(e: E) -> tide::Error {
    tide::Error::new(StatusCode::BadRequest, e)
}

fn to_badgateway<E: Into<anyhow::Error> + Send + 'static + Sync + Debug>(e: E) -> tide::Error {
    log::warn!("bad upstream: {:#?}", e);
    tide::Error::new(StatusCode::BadGateway, e)
}

fn notfound() -> tide::Error {
    tide::Error::new(StatusCode::NotFound, anyhow::anyhow!("not found"))
}

/// Get the latest status
#[tracing::instrument]
pub async fn get_latest(req: tide::Request<ValClient>) -> tide::Result<Body> {
    let last_snap = req.state().snapshot().await.map_err(to_badgateway)?;
    Ok(Body::from_json(&last_snap.header())?)
}

/// Get a particular block header
#[tracing::instrument]
pub async fn get_header(req: tide::Request<ValClient>) -> tide::Result<Body> {
    let height: u64 = req.param("height")?.parse().map_err(to_badreq)?;
    let last_snap = req.state().snapshot().await.map_err(to_badgateway)?;
    let older = last_snap.get_history(height).await.map_err(to_badgateway)?;
    Ok(Body::from_json(&older.ok_or_else(notfound)?)?)
}

/// Get a particular transaction
#[tracing::instrument]
pub async fn get_transaction(req: tide::Request<ValClient>) -> tide::Result<Body> {
    let height: u64 = req.param("height")?.parse()?;
    let txhash: String = req.param("txhash")?.into();
    let txhash: Vec<u8> = hex::decode(&txhash)?;
    let txhash: HashVal = HashVal(
        txhash
            .try_into()
            .map_err(|_| anyhow::anyhow!("not the right length"))?,
    );
    let last_snap = req.state().snapshot().await?;
    let older = last_snap.get_older(height).await?;
    let tx = older.get_transaction(txhash).await?;
    Ok(Body::from_json(&tx.ok_or_else(notfound)?)?)
}
