#![feature(map_first_last)]

mod common;
pub use common::*;
mod machine;
pub use machine::*;
mod pacemaker;
pub mod testing;
use async_trait::async_trait;
pub use pacemaker::*;

pub type DestMsg = (Option<tmelcrypt::Ed25519PK>, SignedMessage);

/// Decider is an object-safe trait abstracts over all Themelio-compatible consensus algorithms. The choice of Decider is ``soft-critical'': more than 2/3 of stakeholders must agree on which Decider to use, but the choice of Decider does not affect that core state-transition function, and problems in Decider do not affact economic finality, since slashing and other incentives are decided by the state-transition function.
///
/// Currently, Pacemaker is the only implementation of Decider in this crate, though other crates may implement Decider freely.

#[async_trait]
pub trait Decider {
    /// Waits for the next output message.
    async fn next_output(&self) -> DestMsg;
    /// Waits for the decision
    async fn decision(&self) -> QuorumCert;
    /// Processes an input message, possibly unblocking next_output or decision.
    fn process_input(&self, msg: SignedMessage);
}

/*
#[cfg(test)]
mod tests {
    use super::*;
    use futures::channel::mpsc;
    use futures::prelude::*;
    use futures::task::SpawnExt;
    use std::sync::Arc;

    #[test]
    fn one_party_trivial() {
        let _ = env_logger::try_init();
        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let config = Config {
            sender_weight: Arc::new(|_| 1.0),
            view_leader: Arc::new(move |_| pk),
            is_valid_prop: Arc::new(|_| true),
            gen_proposal: Arc::new(|| b"Hello World".to_vec()),
            my_sk: sk,
            my_pk: pk,
        };
        let machine = Machine::new(config);
        assert!(machine.decision().is_some());
    }

    #[smol_potat::test]
    async fn multi_party_pacemaker() {
        let _ = env_logger::try_init();
        const PARTIES: usize = 10;
        // create the keypairs
        let keypairs: Vec<_> = (0..PARTIES).map(|_| tmelcrypt::ed25519_keygen()).collect();
        // config
        let config_gen = {
            let keypairs = keypairs.clone();
            |sk, pk| Config {
                sender_weight: Arc::new(move |_| 1.0 / (PARTIES as f64)),
                view_leader: Arc::new(move |view| keypairs[(view as usize) % keypairs.len()].0),
                is_valid_prop: Arc::new(|_| true),
                gen_proposal: Arc::new(|| b"Hello World".to_vec()),
                my_pk: pk,
                my_sk: sk,
            }
        };
        // create the pacemakers
        let pacers: Vec<Pacemaker> = (0..PARTIES)
            .map(|i| {
                let config_gen = config_gen.clone();
                let cfg = config_gen(keypairs[i].1, keypairs[i].0);
                Pacemaker::new(Machine::new(cfg))
            })
            .collect();
        let pacers = Arc::new(pacers);
        // background task to shuffle things around
        let send_broadcast = {
            let (send_broadcast, mut recv_broadcast) = mpsc::unbounded::<SignedMessage>();
            let pacers = pacers.clone();
            smol::Task::spawn(async move {
                loop {
                    let msg: SignedMessage = recv_broadcast.next().await.unwrap();
                    for p in pacers.iter() {
                        p.process_input(msg.clone());
                    }
                }
            })
            .detach();
            send_broadcast
        };
        // drain output of pacemakers
        (0..PARTIES).for_each(|i| {
            let pacers = pacers.clone();
            let send_broadcast = send_broadcast.clone();
            smol::Task::spawn(async move {
                loop {
                    let output = pacers[i].next_output();
                    let (_, out) = output.await;
                    send_broadcast.unbounded_send(out).unwrap();
                }
            })
            .detach();
        });
        // wait for decisions
        for p in pacers.iter() {
            p.decision().await;
        }
    }
}
*/
