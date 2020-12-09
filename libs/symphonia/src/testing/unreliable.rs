use log::trace;
use rand::prelude::*;
use rand_distr::{Distribution, Normal};
use smol::channel::{Receiver, Sender};

use super::MockNet;
use std::time::Duration;

/// Creates an unbounded unreliable channel with the given MockNet parameters.
#[allow(clippy::clippy::type_complexity)]
pub fn unbounded<T: Send + 'static>(network: MockNet) -> (Sender<T>, Receiver<T>) {
    let (input, recv_input) = smol::channel::unbounded();
    let (send_output, output) = smol::channel::unbounded();

    // we give this a type so that we can use ?
    let _task: smol::Task<Option<()>> = smolscale::spawn(async move {
        // we just return whenever the channels are closed. this ensures that the background task doesn't leak
        loop {
            // read a message into lossy output channel
            let output = recv_input.recv().await.ok()?;

            // drop it based on loss probability
            if rand::thread_rng().gen::<f64>() < network.loss_prob {
                trace!("Simulated loss");
                continue;
            }

            // compute delay duration from latency params using a standard curve
            let normal =
                Normal::new(network.latency_mean_ms, network.latency_standard_deviation).unwrap();
            let delay = normal.sample(&mut rand::thread_rng()).round() as u64;
            trace!("[delay in ms={}]", delay.clone());
            let delay = Duration::from_millis(delay);

            // send it over later. this is a little inefficient but it's fine because we don't need crazy throughput or anything and tasks are cheap
            let send_output = send_output.clone();
            smolscale::spawn(async move {
                smol::Timer::after(delay).await;
                // ignore error
                let _ = send_output.send(output).await;
            })
            .detach();
        }
    });
    _task.detach();
    (input, output)
}
