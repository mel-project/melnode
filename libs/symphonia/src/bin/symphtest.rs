use env_logger::Env;
use rand::Rng;
use serde::Deserialize;
use std::env;
use std::fs;
use structopt::StructOpt;
use symphonia::testing::{Harness, MockNet};

#[derive(Debug, StructOpt, Clone)]
#[structopt(
    name = "Symphonia test harness",
    about = "Simulate a network of nodes running Symphonia"
)]
enum Opt {
    #[structopt(about = "Simulate rounds in Symphonia for a set of params")]
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

    #[structopt(about = "Simulate different test cases selection params from input file")]
    Parameterize {
        #[structopt(
            name = "test-count",
            long,
            short,
            help = "Number of test simulations to run",
            default_value = "1"
        )]
        test_count: u64,

        #[structopt(
            name = "rounds",
            long,
            short,
            help = "Number of rounds or times to reach consensus on a block",
            default_value = "1"
        )]
        rounds: u64,

        #[structopt(
            name = "filename",
            long,
            short,
            help = "Input params file name containing to determine values to test"
        )]
        file_name: String,
    },
}

#[derive(Debug, Deserialize)]
struct NetworkLatencyParams {
    mean_milli_sec: Vec<f64>,
    standard_deviation: Vec<f64>,
    loss_probability: Vec<f64>,
}

impl NetworkLatencyParams {
    /// Calculate and return a sample from min and max on latency fields
    fn sample(&self) -> (f64, f64, f64) {
        let mut rng = rand::thread_rng();
        let mean = rng.gen_range(self.mean_milli_sec[0], self.mean_milli_sec[1]);
        let standard_deviation =
            rng.gen_range(self.standard_deviation[0], self.standard_deviation[1]);
        let loss_probability = rng.gen_range(self.loss_probability[0], self.loss_probability[1]);
        (mean, standard_deviation, loss_probability)
    }
}

#[derive(Debug, Deserialize)]
struct ParticipantParams {
    pareto_alpha: Vec<f64>,
    num_participants: Vec<u64>,
}

impl ParticipantParams {
    fn sample(&self) -> Vec<u64> {
        // TODO: sample pareto alpha and skew the voting weight per participant
        let mut rng = rand::thread_rng();
        let num_participants = rng.gen_range(self.num_participants[0], self.num_participants[1]);
        vec![100; num_participants as usize]
    }
}

#[derive(Debug, Deserialize)]
struct Params {
    latency: NetworkLatencyParams,
    participants: ParticipantParams,
}

fn main() {
    env_logger::from_env(Env::default().default_filter_or("symphonia=trace,warn")).init();
    let opt: Opt = Opt::from_args();
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
                rounds,
                file_name,
                test_count,
            } => {
                // Load file and deserialize into params
                let mut path = env::current_dir().expect("Failed to get current directory");
                path.push(file_name);
                let file_contents = fs::read_to_string(path).expect("Unable to read file");
                let params: Params =
                    toml::from_str(&file_contents).expect("Unable to deserialize params");

                // Run test cases
                for _ in 0..test_count {
                    // Sample latency and create mock network
                    let (latency_mean_ms, latency_standard_deviation, loss_prob) =
                        params.latency.sample();
                    let mock_net = MockNet {
                        latency_mean_ms,
                        loss_prob,
                        latency_standard_deviation,
                    };

                    // Sample participants and run rounds
                    let participant_weights = params.participants.sample();
                    for _ in 0..rounds {
                        run_round(participant_weights.clone(), mock_net.clone()).await
                    }
                }
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
