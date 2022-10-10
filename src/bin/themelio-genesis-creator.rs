use structopt::StructOpt;
use themelio_structs::NetID;

#[derive(Debug, StructOpt)]
pub struct Args {
    #[structopt(long)]
    _netid: NetID,
}

fn main() {
    let _opts = Args::from_args();
}
