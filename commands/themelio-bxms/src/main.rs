use std::{convert::TryInto, net::SocketAddr};

use blkstructs::NetID;
use nodeprot::ValClient;
use once_cell::sync::OnceCell;
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

pub static GLOB_CLIENT: OnceCell<ValClient> = OnceCell::new();

async fn main_inner() -> anyhow::Result<()> {
    let args = Args::from_args();
    let client = ValClient::new(NetID::Testnet, args.connect);
    // TODO read this from an argument
    client.trust(
        70000,
        HashVal(
            hex::decode("8e45c78d6cb6f70cc2ac91b9cefe6844ce98037b6254fa838cb94a650d923c70")?
                .try_into()
                .unwrap(),
        ),
    );
    GLOB_CLIENT
        .set(client)
        .expect("GLOB_CLIENT should only be initialized once");
    let mut app = tide::new();
    app.at("/raw/latest").get(raw::get_latest);
    app.listen(args.listen).await?;
    Ok(())
}
