use std::{convert::TryInto, net::SocketAddr};

use blkstructs::NetID;
use nodeprot::ValClient;
use structopt::StructOpt;
use tmelcrypt::HashVal;

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
        14045,
        HashVal(
            hex::decode("26115e62677743abf10210a15e8be8984e63f3a34fb43ff11907e885814ef9cd")?
                .try_into()
                .unwrap(),
        ),
    );
    let mut app = tide::with_state(client);
    app.at("/raw/latest").get(raw::get_latest);
    app.at("/raw/blocks/:height").get(raw::get_header);
    app.at("/raw/blocks/:height/transactions/:txhash")
        .get(raw::get_transaction);
    tracing::info!("Starting REST endpoint at {}", args.listen);
    app.listen(args.listen).await?;
    Ok(())
}
