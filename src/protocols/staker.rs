use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime, SystemTimeError},
};

use crate::{common::insecure_testnet_keygen, SharedStorage};
use blkstructs::{Block, StakeMapping, Transaction, STAKE_EPOCH};
use melnet::Request;
use smol::channel::{Receiver, Sender};
use smol::prelude::*;
use symphonia::Decider;
use tmelcrypt::{Ed25519PK, Ed25519SK};

const NETNAME: &str = "testnet-staker";

/// This encapsulates the staker-specific peer-to-peer. At the moment it's very naive, directly using symphonia with blocks, but it can be improved without disrupting the rest of the code.
pub struct StakerProtocol {
    _network_task: smol::Task<()>,
}

impl StakerProtocol {
    /// Creates a new instance of the staker protocol.
    pub fn new(
        addr: SocketAddr,
        bootstrap: Vec<SocketAddr>,
        state: SharedStorage,
        my_sk: Ed25519SK,
    ) -> anyhow::Result<Self> {
        let network = melnet::NetState::new_with_name(NETNAME);
        for addr in bootstrap {
            network.add_route(addr);
        }
        network.add_route(addr);
        let responder = StakerResponder::new(network.clone(), state, my_sk);
        network.register_verb("symphonia", responder);
        let _network_task = smolscale::spawn(async move {
            network
                .run_server(smol::net::TcpListener::bind(addr).await.unwrap())
                .await
        });
        Ok(Self { _network_task })
    }
}

struct StakerResponder {
    storage: SharedStorage,
    send_input: Sender<symphonia::SignedMessage>,
}

impl StakerResponder {
    fn new(network: melnet::NetState, storage: SharedStorage, my_sk: Ed25519SK) -> Self {
        let (send_input, recv_input) = smol::channel::unbounded();
        smolscale::spawn(staker_loop(network, storage.clone(), recv_input, my_sk)).detach();
        Self {
            storage,
            send_input,
        }
    }
}

async fn staker_loop(
    network: melnet::NetState,
    storage: SharedStorage,
    recv_input: Receiver<symphonia::SignedMessage>,
    my_sk: Ed25519SK,
) -> anyhow::Result<()> {
    loop {
        next_consensus_time().await;
        let proposal = Arc::new(storage.read().curr_state.clone().finalize());
        let height = proposal.inner_ref().height;
        let stakes: StakeMapping = proposal.inner_ref().stakes.clone();
        // create a configuration
        let symphonia_config = symphonia::Config {
            // sender weight is computed by using the stake mapping
            sender_weight: {
                let stakes = stakes.clone();
                Arc::new(move |pk: Ed25519PK| {
                    // look up in the stake info
                    stakes.vote_power(height / STAKE_EPOCH, pk)
                })
            },
            // view_leader right now is hardcoded
            view_leader: { Arc::new(move |_view: u64| insecure_testnet_keygen(0).0) },
            is_valid_prop: {
                Arc::new(|_prop_msg: &[u8]| {
                    log::debug!("is_valid_prop forcing true");
                    true
                })
            },
            gen_proposal: {
                let blk = proposal.to_block();
                Arc::new(move || bincode::serialize(&blk).unwrap())
            },
            my_sk,
            my_pk: my_sk.to_public(),
        };
        let machine = symphonia::Machine::new(symphonia_config);
        let pacemaker = symphonia::Pacemaker::new(machine);
        // drive the pacemaker
        enum Evt {
            Incoming(symphonia::SignedMessage),
            Outgoing(symphonia::DestMsg),
            Decision(symphonia::QuorumCert),
            OobDecision,
        }
        let decision = loop {
            let incoming_evt = async {
                let msg = recv_input.recv().await?;
                Ok::<_, anyhow::Error>(Evt::Incoming(msg))
            };
            let outgoing_evt = async {
                let msg = pacemaker.next_output().await;
                Ok(Evt::Outgoing(msg))
            };
            let decision_evt = async {
                let decision = pacemaker.decision().await;
                Ok::<_, anyhow::Error>(Evt::Decision(decision))
            };
            let oob_evt = async {
                loop {
                    let last_blk = storage.read().last_block();
                    if let Some(last_blk) = last_blk {
                        if last_blk.inner().header().height >= height {
                            log::warn!("symphonia cancelled out of band");
                            break Ok::<_, anyhow::Error>(Evt::OobDecision);
                        }
                    }
                    smol::Timer::after(Duration::from_millis(100)).await;
                }
            };
            let evt: Evt = decision_evt
                .or(incoming_evt)
                .or(outgoing_evt)
                .or(oob_evt)
                .await?;
            match evt {
                Evt::Incoming(msg) => pacemaker.process_input(msg),
                Evt::Outgoing(dmsg) => {
                    smolscale::spawn(symphonia_mcast(network.clone(), dmsg)).detach()
                }
                Evt::Decision(decision) => break Some(decision),
                Evt::OobDecision => break None,
            }
        };
        // DECISION!
        if let Some(decision) = decision {
            log::debug!("DECISION REACHED! Committing to storage...");
            let blk: Block = bincode::deserialize(&decision.node.prop)
                .expect("symphonia decided on an invalidly formatted block");
            storage
                .write()
                .apply_block(blk, decision)
                .expect("unable to apply just-decided block to storage!");
        }
    }
}

/// multicasts a message
#[tracing::instrument(skip(network, dmsg))]
async fn symphonia_mcast(
    network: melnet::NetState,
    dmsg: symphonia::DestMsg,
) -> anyhow::Result<()> {
    // TODO: more intelligent routing
    let (_dest, msg) = dmsg;
    for route in network.routes() {
        log::debug!("mcast {:?} to {}", msg.msg.phase, route);
        smolscale::spawn(melnet::g_client().request::<_, ()>(
            route,
            NETNAME,
            "symphonia",
            msg.clone(),
        ))
        .detach();
    }
    Ok(())
}

impl melnet::Responder<symphonia::SignedMessage, ()> for StakerResponder {
    fn respond(&mut self, req: Request<symphonia::SignedMessage, ()>) {
        let _ = self.send_input.try_send(req.body.clone());
        req.respond(Ok(()))
    }
}

/// Wait until the next consensus
async fn next_consensus_time() {
    let now_unix = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let cons_unix = Duration::from_secs((now_unix.as_secs() / 30 + 1) * 30);
    assert!(cons_unix > now_unix);
    log::debug!("waiting till next consensus time {}", cons_unix.as_secs());
    while SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        < cons_unix
    {
        // wait in a loop to wait "clock time" rather than real time
        smol::Timer::after(Duration::from_secs(1)).await;
    }
}
