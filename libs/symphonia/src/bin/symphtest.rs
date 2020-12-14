use env_logger::Env;
use structopt::StructOpt;
use symphonia::testing::{Harness, MockNet};

#[derive(Debug, StructOpt)]
#[structopt(
    name = "Symphonia Test Harness",
    about = "Simulate a network of nodes running Symphonia"
)]
struct Opt {
    #[structopt(
        name = "mean",
        long,
        short,
        help = "Mean time in ms for latency",
        default_value = "100.0"
    )]
    latency_mean_ms: f64,

    #[structopt(
        name = "variance",
        long,
        short,
        help = "Variance of normal distribution for latency",
        default_value = "20.0"
    )]
    latency_variance: f64,

    #[structopt(
        name = "loss",
        long,
        short,
        help = "Probability of loss per network transfer",
        default_value = "0.05"
    )]
    loss_prob: f64,

    #[structopt(
        name = "weights",
        long,
        short,
        help = "Comma separated voting weight of each consensus participants",
        default_value = "100",
        use_delimiter = true
    )]
    participant_weights: Vec<u64>,
}

fn main() {
    let opt: Opt = Opt::from_args();
    println!("{:?}", opt);
    smol::block_on(async move {
        env_logger::from_env(Env::default().default_filter_or("symphonia=trace,warn")).init();
        let mut harness = Harness::new(MockNet {
            latency_mean_ms: opt.latency_mean_ms,
            latency_variance: opt.latency_variance,
            loss_prob: opt.loss_prob,
        });
        for participant_weight in opt.participant_weights.iter() {
            harness = harness.add_participant(tmelcrypt::ed25519_keygen().1, *participant_weight);
        }
        harness.run().await
    });
}
