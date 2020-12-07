use env_logger::Env;
use std::path::PathBuf;
use std::time::Duration;
use structopt::StructOpt;
use symphonia::testing::{Harness, MockNet};

#[derive(Debug, StructOpt)]
#[structopt(
    name = "Symphonia Test Harness",
    about = "Simulate a network of nodes running symphonia"
)]
struct Opt {
    #[structopt(
        name = "mean",
        long,
        short,
        help = "Mean time in ms for latency",
        default = "100"
    )]
    latency_mean_ms: u32,

    #[structopt(
        name = "variance",
        long,
        short,
        help = "Variance time in ms for latency",
        default = "10"
    )]
    latency_variance_ms: u32,

    #[structopt(
        name = "loss",
        long,
        short,
        help = "Probability of loss per network transfer",
        default = "0.01"
    )]
    loss_prob: f64,

    #[structopt(
        name = "weights",
        long,
        short,
        help = "Comma separated voting weight of each consensus participants",
        default_value = "100",
        raw(use_delimiter = "true")
    )]
    participant_weights: Vec<u32>,
}

fn main() {
    smol::block_on(async move {
        env_logger::from_env(Env::default().default_filter_or("symphonia=trace,warn")).init();
        let mut harness = Harness::new(MockNet {
            latency_mean: Duration::from_millis(100),
            latency_variance: Duration::from_millis(10),
            loss_prob: 0.01,
        });
        for _ in 0..100 {
            harness = harness.add_participant(tmelcrypt::ed25519_keygen().1, 100);
        }
        harness.run().await
    });
}
