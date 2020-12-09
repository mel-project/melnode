use env_logger::Env;
use structopt::StructOpt;
use symphonia::testing::{Harness, MockNet};

#[derive(Debug, StructOpt, Clone)]
#[structopt(
    name = "Symphonia Test Harness",
    about = "Simulate a network of nodes running Symphonia for a number of rounds"
)]
struct Opt {
    #[structopt(
        name = "rounds",
        long,
        short,
        help = "Number of rounds or times to reach consensus on a block",
        default_value = "1"
    )]
    rounds: u64,

    #[structopt(
        name = "mean",
        long,
        short,
        help = "Mean time in ms for latency",
        default_value = "100.0"
    )]
    latency_mean_ms: f64,

    #[structopt(
        name = "deviation",
        long,
        short,
        help = "Standard deviation of normal distribution for latency",
        default_value = "5.0"
    )]
    latency_standard_deviation: f64,

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
    env_logger::from_env(Env::default().default_filter_or("symphonia=trace,warn")).init();
    let opt: Opt = Opt::from_args();
    let mock_net = MockNet {
        latency_mean_ms: opt.latency_mean_ms,
        latency_standard_deviation: opt.latency_standard_deviation,
        loss_prob: opt.loss_prob,
    };
    println!("{:?}", opt);
    smol::block_on(async move {
        for _ in 0..opt.rounds {
            run_round(opt.clone(), mock_net).await;
        }
    });
}

async fn run_round(opt: Opt, mock_net: MockNet) {
    let mut harness = Harness::new(mock_net);
    for participant_weight in opt.participant_weights.iter() {
        harness =
            harness.add_participant(tmelcrypt::ed25519_keygen().1, participant_weight.clone());
    }
    harness.run().await
}
