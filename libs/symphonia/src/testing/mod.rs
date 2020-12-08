use crate::{Config, Decider, Machine, Pacemaker, SignedMessage};
use rand::Rng;
use smol::channel::{Receiver, Sender};
use smol::prelude::*;
use std::{collections::HashMap, sync::Arc, thread, time::Duration};
use tmelcrypt::Ed25519PK;

/// A harness for testing that uses a mock network to transport messages. Uses a builder-style pattern and should be "run" at the end.
pub struct Harness {
    network: MockNet,
    participants: Vec<(tmelcrypt::Ed25519SK, u64)>,
}

impl Harness {
    /// Create a new harness with the given network config.
    pub fn new(network: MockNet) -> Self {
        Harness {
            network,
            participants: Vec::new(),
        }
    }
    /// Adds a new participant, represented by a secret key and a vote weight.
    pub fn add_participant(mut self, sk: tmelcrypt::Ed25519SK, weight: u64) -> Self {
        self.participants.push((sk, weight));
        self
    }
    /// Runs the harness until all honest participants decide.
    pub async fn run(self) {
        let (send_global, recv_global) = Harness::unreliable_channel(self.network.clone());
        let num_participants = self.participants.len();
        let total_weight: u64 = self.participants.iter().map(|(_, w)| w).sum();
        let weight_map: HashMap<tmelcrypt::Ed25519PK, f64> = self
            .participants
            .iter()
            .map(|(sk, w)| (sk.to_public(), *w as f64 / total_weight as f64))
            .collect();
        let sender_weight = Arc::new(move |pk: tmelcrypt::Ed25519PK| {
            weight_map.get(&pk).cloned().unwrap_or_default()
        });
        let pp = self.participants.clone();
        let view_leader = Arc::new(move |view: u64| pp[view as usize % pp.len()].0.to_public());
        let is_valid_prop = Arc::new(|prop: &[u8]| prop[0] % 2 == 0);
        let gen_proposal = Arc::new(|| String::from("nuuunuuNUUU!").as_bytes().to_vec());
        let mut pacemakers = HashMap::new();
        let (send_decision, recv_decision) = smol::channel::unbounded();
        for (sk, _) in self.participants {
            let cfg = Config {
                sender_weight: sender_weight.clone(),
                view_leader: view_leader.clone(),
                is_valid_prop: is_valid_prop.clone(),
                gen_proposal: gen_proposal.clone(),
                my_sk: sk,
                my_pk: sk.to_public(),
            };
            let machine = Machine::new(cfg);
            let pmaker = Arc::new(Pacemaker::new(machine));
            // output waiter
            let fut_out_wait = {
                let pmaker = pmaker.clone();
                let send_global = send_global.clone();
                async move {
                    loop {
                        let output = pmaker.next_output().await;
                        let _ = send_global.send(output).await;
                    }
                }
            };
            // decision waiter
            smolscale::spawn(
                {
                    let pmaker = pmaker.clone();
                    let send_decision = send_decision.clone();
                    async move {
                        let decision = pmaker.decision().await;
                        send_decision.try_send(decision).unwrap();
                    }
                }
                .or(fut_out_wait),
            )
            .detach();
            pacemakers.insert(sk.to_public(), pmaker);
        }
        // message stuffer, drop automatically
        let _stuffer = smolscale::spawn(async move {
            loop {
                let (dest, msg) = recv_global.recv().await.unwrap();
                if let Some(dest) = dest {
                    // there's a definite destination
                    let dest = pacemakers.get(&dest).expect("nonexistent destination");
                    dest.process_input(msg);
                } else {
                    for (_, dest) in pacemakers.iter() {
                        dest.process_input(msg.clone());
                    }
                }
            }
        });
        // time to wait for the decisions
        for _ in 0..num_participants {
            let dec = recv_decision.recv().await.unwrap();
            dbg!(dec);
        }
    }
    // Creates a global sender and receiver channel with a lossy channel in between
    // The lossy channel either drops messages or forwards them with a delay.
    fn unreliable_channel(
        network: MockNet,
    ) -> (
        Sender<(Option<Ed25519PK>, SignedMessage)>,
        Receiver<(Option<Ed25519PK>, SignedMessage)>,
    ) {
        let (send_global, lossy_recv_global) = smol::channel::unbounded();

        let (lossy_send_global, recv_global) = smol::channel::unbounded();

        smolscale::spawn(async move {
            loop {
                // read a message into lossy output channel
                let output = lossy_recv_global.recv().await.unwrap();

                // drop it based on loss probability
                let mut rng = rand::thread_rng();
                if rng.gen::<f64>() > network.loss_prob {
                    continue;
                }

                // or delay and pass it along
                // TODO: add variance range offset
                thread::sleep(network.latency_mean);

                lossy_send_global.send(output);
            }
        })
        .detach();
        (send_global, recv_global)
    }
}

/// A mock-network.
#[derive(Clone, Debug, Copy)]
pub struct MockNet {
    pub latency_mean: Duration,
    pub latency_variance: Duration,
    pub loss_prob: f64,
}

/// An efficient lossy channel.
///
/// Elements can be stuffed in, and they will be delayed until a given time or lost before they can be read out. This simulates a bad network connection or other similar construct.
pub struct LossyChan;
