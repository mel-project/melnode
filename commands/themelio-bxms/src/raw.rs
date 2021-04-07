use std::convert::TryInto;

use nodeprot::ValClient;
use tide::Body;
use tmelcrypt::HashVal;

/// Get the latest status
#[tracing::instrument]
pub async fn get_latest(req: tide::Request<ValClient>) -> tide::Result<Body> {
    let last_snap = req.state().snapshot().await?;
    Ok(Body::from_json(&last_snap.header())?)
}

/// Get a particular block header
#[tracing::instrument]
pub async fn get_header(req: tide::Request<ValClient>) -> tide::Result<Body> {
    let height: u64 = req.param("height")?.parse()?;
    let last_snap = req.state().snapshot().await?;
    let older = last_snap.get_history(height).await?;
    Ok(Body::from_json(&older)?)
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
    Ok(Body::from_json(&tx)?)
}
