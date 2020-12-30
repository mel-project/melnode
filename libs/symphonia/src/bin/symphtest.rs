use env_logger::Env;
use rand::prelude::*;
use serde::Deserialize;
use smol::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use structopt::StructOpt;
use symphonia::testing::{Harness, MetricsGatherer, MockNet, TestResult};

#[derive(Debug, StructOpt, Clone)]
#[structopt(
    name = "Symphonia test harness",
    about = "Simulate a network of nodes running Symphonia"
)]
enum Opt {
    TestCase(TestCaseOpt),
    TestCases(TestCasesOpt),
}

#[derive(Debug, StructOpt, Clone)]
#[structopt(about = "Run Symphonia harness consecutively for a set of params")]
struct TestCaseOpt {
    #[structopt(
        name = "run-count",
        long,
        short,
        help = "Number of times to run harness to reach consensus on a block",
        default_value = "1"
    )]
    run_count: u64,

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

#[derive(Debug, StructOpt, Clone)]
#[structopt(about = "Simulate different test cases selection params from input file")]
struct TestCasesOpt {
    #[structopt(
        name = "test-count",
        long,
        short,
        help = "Number of test cases to run",
        default_value = "1"
    )]
    test_count: u64,

    #[structopt(
        name = "run-count",
        long,
        short,
        help = "Number of times to run test harness for a test case",
        default_value = "1"
    )]
    run_count: u64,

    #[structopt(
        name = "filename",
        long,
        short,
        help = "Input params file name containing to determine values to test"
    )]
    file_name: String,
}

#[derive(Debug, Deserialize)]
struct NetParams {
    latency_mean_ms: Vec<f64>,
    latency_std_dev: Vec<f64>,
    loss_probability: Vec<f64>,
}

impl NetParams {
    /// Calculate and return a sample from min and max on latency fields
    fn sample(&self) -> (f64, f64, f64) {
        let mut rng = rand::thread_rng();
        let mean = rng.gen_range(self.latency_mean_ms[0], self.latency_mean_ms[1]);
        let standard_deviation = rng.gen_range(self.latency_std_dev[0], self.latency_std_dev[1]);
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
    network: NetParams,
    participants: ParticipantParams,
}

fn main() {
    env_logger::from_env(Env::default().default_filter_or("symphonia=trace,warn")).init();
    let opt: Opt = Opt::from_args();
    println!("{:?}", opt);
    smol::block_on(async move {
        match opt {
            Opt::TestCase(test_case) => {
                for _ in 0..test_case.run_count {
                    let mock_net = MockNet {
                        latency_mean_ms: test_case.latency_mean_ms,
                        latency_standard_deviation: test_case.latency_standard_deviation,
                        loss_prob: test_case.loss_prob,
                    };
                    // TODO: avoid clone by using immutable vector conversion before loop
                    run_harness(test_case.participant_weights.clone(), mock_net).await
                }
            }
            Opt::TestCases(test_cases) => {
                // Load file and deserialize into params
                let mut path = std::env::current_dir().expect("Failed to get current directory");
                path.push(test_cases.file_name);
                let file_contents = std::fs::read_to_string(path).expect("Unable to read file");
                let params: Params =
                    toml::from_str(&file_contents).expect("Unable to deserialize params");

                println!("{}", TestResult::header());

                // Run test case
                for _ in 0..test_cases.test_count {
                    // Sample latency and create mock network
                    let (latency_mean_ms, latency_standard_deviation, loss_prob) =
                        params.network.sample();
                    let mock_net = MockNet {
                        latency_mean_ms,
                        loss_prob,
                        latency_standard_deviation,
                    };

                    // Sample participants and run harness based on run count
                    let participant_weights = params.participants.sample();
                    for _ in 0..test_cases.run_count {
                        run_harness(participant_weights.clone(), mock_net).await
                    }
                }
            }
        }
    });
}

async fn run_harness(participant_weights: Vec<u64>, mock_net: MockNet) {
    let mut harness = Harness::new(mock_net);
    for participant_weight in participant_weights.iter() {
        harness = harness.add_participant(tmelcrypt::ed25519_keygen().1, *participant_weight);
    }
    let metrics_gatherer = Arc::new(MetricsGatherer::default());
    let success_fut = async {
        harness.run(metrics_gatherer.clone()).await;
        true
    };
    let fail_fut = async {
        smol::Timer::after(Duration::from_secs(70)).await;
        false
    };
    let succeeded = success_fut.race(fail_fut).await;

    let test_result = metrics_gatherer.summarize().await;
    let test_result_content = test_result.generate(0, succeeded, mock_net, participant_weights);

    println!("{}", test_result_content);
}
