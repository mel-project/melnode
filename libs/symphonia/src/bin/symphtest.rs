use env_logger::Env;
use structopt::StructOpt;
use symphonia::testing::{Harness, MockNet};

#[derive(Debug, StructOpt, Clone)]
#[structopt(
    name = "Symphonia test harness",
    about = "Simulate a network of nodes running Symphonia"
)]
enum Opt {
    #[structopt(about = "Simulate rounds in Symphonia")]
    Rounds {
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
    },

    #[structopt(about = "Simulate different parameter ranges in Symphonia")]
    Parameterize {
        #[structopt(
            name = "rounds",
            long,
            short,
            help = "Comma separated list of min, interval and max round range",
            min_values = 3,
            max_values = 3,
            use_delimiter = true,
            required = true
        )]
        rounds_range: Vec<u64>,

        #[structopt(
            name = "mean",
            long,
            short,
            help = "Comma separated list of min, interval and max mean range",
            min_values = 3,
            max_values = 3,
            use_delimiter = true,
            required = true
        )]
        mean_range: Vec<f64>,

        #[structopt(
            name = "deviation",
            long,
            short,
            help = "Comma separated list of min, interval and max standard deviation range",
            min_values = 3,
            max_values = 3,
            use_delimiter = true,
            required = true
        )]
        standard_deviation_range: Vec<f64>,

        #[structopt(
            name = "loss",
            long,
            short,
            help = "Comma separated list of min, interval and max loss range",
            min_values = 3,
            max_values = 3,
            use_delimiter = true,
            required = true
        )]
        loss_range: Vec<f64>,

        #[structopt(
            name = "pareto",
            long,
            short,
            help = "Comma separated list of min, interval and max pareto alpha scalar range",
            min_values = 3,
            max_values = 3,
            use_delimiter = true,
            required = true
        )]
        pareto_alpha_range: Vec<f64>,

        #[structopt(
            name = "num_participants",
            long,
            short,
            help = "Comma separated list of min, interval and max num of participants range",
            min_values = 3,
            max_values = 3,
            use_delimiter = true,
            required = true
        )]
        num_participants_range: Vec<u64>,
    },
}

fn main() {
    env_logger::from_env(Env::default().default_filter_or("symphonia=trace,warn")).init();
    let opt: Opt = Opt::from_args();
    println!("{:?}", opt);
    smol::block_on(async move {
        match opt {
            Opt::Rounds {
                rounds,
                latency_mean_ms,
                latency_standard_deviation,
                loss_prob,
                participant_weights,
            } => {
                for _ in 0..rounds {
                    let mock_net = MockNet {
                        latency_mean_ms,
                        latency_standard_deviation,
                        loss_prob,
                    };
                    // TODO: avoid clone by using immutable vector conversion before loop
                    run_round(participant_weights.clone(), mock_net).await
                }
            }
            Opt::Parameterize {
                rounds_range,
                mean_range,
                standard_deviation_range,
                loss_range,
                pareto_alpha_range,
                num_participants_range,
            } => {
                let latency_mean_ms = 100.0;
                let latency_standard_deviation = 5.0;
                let loss_prob = 0.01;
                let participant_weights = vec![100, 100, 100, 100, 100, 100, 100, 100, 100];
                for round_range in rounds_range {
                    for _ in 0..round_range {
                        let mock_net = MockNet {
                            latency_mean_ms,
                            latency_standard_deviation,
                            loss_prob,
                        };
                        // TODO: avoid clone by using immutable vector conversion before loop
                        run_round(participant_weights.clone(), mock_net).await
                    }
                }

                // run_rounds(rounds_range.clone(), latency_mean_ms, latency_standard_deviation, loss_prob, participant_weights).await;
            }
        }
    });
}

async fn run_round(participant_weights: Vec<u64>, mock_net: MockNet) {
    let mut harness = Harness::new(mock_net);
    for participant_weight in participant_weights.iter() {
        harness =
            harness.add_participant(tmelcrypt::ed25519_keygen().1, participant_weight.clone());
    }
    harness.run().await
}

async fn run_rounds() {}
