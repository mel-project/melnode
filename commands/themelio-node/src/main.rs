use structopt::StructOpt;
mod config;
mod dal;
mod protocols;
mod services;
mod tasks;
use tasks::NodeConfig;

use tracing::instrument;

#[instrument]
fn main() {
    // LogTracer::init().unwrap();
    let log_conf = std::env::var("RUST_LOG").unwrap_or_else(|_| "themelio_node=debug,warn".into());
    std::env::set_var("RUST_LOG", log_conf);
    tracing_subscriber::fmt::init();
    // env_logger::Builder::from_env("THEMELIO_LOG")
    //     .parse_filters("themelio_core")
    //     .init();
    let opts = NodeConfig::from_args();
    smolscale::block_on(tasks::run_node(opts))
}
