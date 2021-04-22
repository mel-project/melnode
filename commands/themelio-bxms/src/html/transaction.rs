use crate::{to_badgateway, to_badreq};
use askama::Template;
use nodeprot::ValClient;
use tmelcrypt::HashVal;

use super::RenderTimeTracer;

#[derive(Template)]
#[template(path = "transaction.html")]
struct TransactionTemplate {
    txhash: HashVal,
}

pub async fn get_txpage(req: tide::Request<ValClient>) -> tide::Result<tide::Body> {
    let _render = RenderTimeTracer::new("blockpage");

    let height: u64 = req.param("height").unwrap().parse().map_err(to_badreq)?;
    let txhash: HashVal = req.param("txhash").unwrap().parse().map_err(to_badreq)?;
    let mut body: tide::Body = TransactionTemplate { txhash }.render().unwrap().into();
    body.set_mime("text/html");
    Ok(body)
}
