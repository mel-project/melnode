use structopt::StructOpt;
use themelio_core::*;

fn main() {
    env_logger::Builder::from_env("THEMELIO_LOG")
        .parse_filters("themelio_core")
        .init();
    let opts = Config::from_args();
    smol::run(run_main(opts))
}
