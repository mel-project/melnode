use std::time::Duration;

use env_logger::Env;
use symphonia::testing::{Harness, MockNet};

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
