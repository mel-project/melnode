use structopt::StructOpt;
use themelio_core::*;
use tracing::instrument;
use tracing_log::LogTracer;

#[instrument]
fn main() {
    // LogTracer::init().unwrap();
    let log_conf = std::env::var("RUST_LOG").unwrap_or_else(|_| "themelio_core=debug,warn".into());
    std::env::set_var("RUST_LOG", log_conf);
    tracing_subscriber::fmt::init();
    // env_logger::Builder::from_env("THEMELIO_LOG")
    //     .parse_filters("themelio_core")
    //     .init();
    let opts = Config::from_args();
    smol::block_on(run_main(opts))
}
