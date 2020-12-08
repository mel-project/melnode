use rand::prelude::*;
use smol::channel::{Receiver, Sender};

use super::MockNet;

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
            if rand::thread_rng().gen::<f64>() > network.loss_prob {
                continue;
            }

            // send it over later. this is a little inefficient but it's fine because we don't need crazy throughput or anything and tasks are cheap
            let send_output = send_output.clone();
            smolscale::spawn(async move {
                smol::Timer::after(network.latency_mean).await;
                // ignore error
                let _ = send_output.send(output).await;
            })
            .detach();
        }
    });
    _task.detach();
    (input, output)
}
