use std::{convert::TryInto, net::SocketAddr};

use blkstructs::NetID;
use nodeprot::ValClient;
use std::fmt::Debug;
use structopt::StructOpt;
use tide::StatusCode;
use tmelcrypt::HashVal;
mod html;
mod raw;

fn main() -> anyhow::Result<()> {
    smol::block_on(main_inner())
}

#[derive(StructOpt)]
pub struct Args {
    #[structopt(long)]
    /// Where to listen for incoming REST API calls
    listen: SocketAddr,

    #[structopt(long)]
    /// A full node to connect to
    connect: SocketAddr,
}

#[tracing::instrument]
async fn main_inner() -> anyhow::Result<()> {
    let log_conf = std::env::var("RUST_LOG").unwrap_or_else(|_| "themelio_bxms=debug,warn".into());
    std::env::set_var("RUST_LOG", log_conf);
    tracing_subscriber::fmt::init();

    let args = Args::from_args();
    let client = ValClient::new(NetID::Testnet, args.connect);
    // TODO read this from an argument
    client.trust(
        3360,
        HashVal(
            hex::decode("843640b26a4a6e55e9c2ef485412c6af7ebefac2872ea029534abb84f038404f")?
                .try_into()
                .unwrap(),
        ),
    );
    let mut app = tide::with_state(client);
    // Rendered paths
    app.at("/").get(html::get_homepage);
    app.at("/blocks/:height").get(html::get_blockpage);
    app.at("/pools/:denom").get(html::get_poolpage);
    app.at("/blocks/:height/:txhash").get(html::get_txpage);
    // Raw paths
    app.at("/raw/latest").get(raw::get_latest);
    app.at("/raw/blocks/:height").get(raw::get_header);
    app.at("/raw/blocks/:height/full").get(raw::get_full_block);
    app.at("/raw/blocks/:height/transactions/:txhash")
        .get(raw::get_transaction);
    app.at("/raw/blocks/:height/coins/:coinid")
        .get(raw::get_coin);
    app.at("/raw/blocks/:height/pools/:denom")
        .get(raw::get_pool);
    tracing::info!("Starting REST endpoint at {}", args.listen);
    app.listen(args.listen).await?;
    Ok(())
}

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
