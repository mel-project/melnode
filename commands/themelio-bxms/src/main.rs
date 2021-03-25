use std::net::SocketAddr;

use blkstructs::NetID;
use nodeprot::ValClient;
use once_cell::sync::OnceCell;
use structopt::StructOpt;

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
    GLOB_CLIENT
        .set(client)
        .expect("GLOB_CLIENT should only be initialized once");
    let mut app = tide::new();
    app.at("/raw/latest").get(raw::get_latest);
    app.listen(args.listen).await?;
    Ok(())
}
