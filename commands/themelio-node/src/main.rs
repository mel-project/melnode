use std::time::Duration;

use structopt::StructOpt;
mod config;
mod protocols;
mod services;
mod tasks;
use tasks::NodeConfig;

use tracing::instrument;

#[instrument]
fn main() -> anyhow::Result<()> {
    // LogTracer::init().unwrap();
    let log_conf = std::env::var("RUST_LOG").unwrap_or_else(|_| "themelio_node=debug,warn".into());
    std::env::set_var("RUST_LOG", log_conf);
    tracing_subscriber::fmt::init();
    // env_logger::Builder::from_env("THEMELIO_LOG")
    //     .parse_filters("themelio_core")
    //     .init();
    let opts = NodeConfig::from_args();

    // Create a background thread which checks for deadlocks
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        let deadlocks = parking_lot::deadlock::check_deadlock();
        if deadlocks.is_empty() {
            continue;
        }

        println!("{} deadlocks detected", deadlocks.len());
        for (i, threads) in deadlocks.iter().enumerate() {
            println!("Deadlock #{}", i);
            for t in threads {
                println!("Thread Id {:#?}", t.thread_id());
                println!("{:#?}", t.backtrace());
            }
        }
    });

    smolscale::block_on(tasks::run_node(opts))
}
