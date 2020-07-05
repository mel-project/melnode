mod common;
pub use common::*;
mod machine;
pub use machine::*;
mod pacemaker;
pub use pacemaker::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;
    use log::trace;
    use rand::prelude::*;
    use std::sync::Arc;
    use std::thread;
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

    #[test]
    fn multi_party_pacemaker() {
        let _ = env_logger::try_init();
        const PARTIES: usize = 5;
        // create a bunch of channels
        let (bus_send, bus_recv) = unbounded::<(Option<tmelcrypt::Ed25519PK>, SignedMessage)>();
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
        // spawn the threads
        (0..PARTIES)
            .map(|i| {
                let config_gen = config_gen.clone();
                let bus_send = bus_send.clone();
                let bus_recv = bus_recv.clone();
                let (pk, sk) = keypairs[i];
                thread::spawn(move || {
                    let config = config_gen(sk, pk);
                    let m = Machine::new(config.clone());
                    let mut p = Pacemaker::new(m);
                    let mut rng = rand::thread_rng();
                    trace!("*** THREAD STARTED WITH PK = {:?} ***", pk);
                    loop {
                        if let Ok(pair) = p.output_chan().try_recv() {
                            // trace!(
                            //     "{:?} ={:?}({:?})=> {:?} @ output_chan",
                            //     pk,
                            //     pair.1.msg.phase,
                            //     pair.1.msg.view_number,
                            //     pair.0
                            // );

                            bus_send.send(pair).unwrap();
                        }
                        if let Ok((dest, msg)) = bus_recv.try_recv() {
                            // trace!(
                            //     "{:?} ={:?}({:?})=> {:?} @ bus_recv ({:?}",
                            //     msg.msg.sender,
                            //     msg.msg.phase,
                            //     msg.msg.view_number,
                            //     dest,
                            //     pk
                            // );
                            if dest.is_none() || dest.unwrap() == config.my_pk {
                                p.process_input(msg.clone());
                                if dest.is_none() {
                                    bus_send.send((None, msg)).unwrap();
                                }
                            } else {
                                bus_send.send((dest, msg)).unwrap();
                            }
                        }
                        thread::sleep_ms(10);
                        if (p.decision().try_recv().is_ok()) {
                            return;
                        }
                    }
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .for_each(|t| {
                t.join().unwrap();
            });
    }
}
