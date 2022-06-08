use structopt::StructOpt;
use themelio_structs::NetID;

#[derive(Debug, StructOpt)]
pub struct Args {
    #[structopt(long)]
    netid: NetID,
}

fn main() {
    let opts = Args::from_args();
}
