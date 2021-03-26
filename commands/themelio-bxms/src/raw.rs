use tide::Body;

use crate::GLOB_CLIENT;

pub async fn get_latest(_req: tide::Request<()>) -> tide::Result<Body> {
    let client = GLOB_CLIENT.get().unwrap();
    let last_snap = client.snapshot().await.unwrap();
    Ok(Body::from_json(&last_snap.header())?)
}
