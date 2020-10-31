use crate::auditor::{Auditor, AuditorMsg};
use crate::common::*;
use crate::storage::Storage;
use blkstructs::STAKE_EPOCH;
use derive_more::*;
use futures::channel::mpsc;
use futures::select;
use smol::net::TcpListener;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tmelcrypt::*;

const STAKE_NET: &str = "themelio-stakeholder";

#[derive(Deref, Clone)]
pub struct Stakeholder(Arc<Actor<StakeholderMsg>>);

impl Stakeholder {
    /// Constructs a new Stakeholder.
    pub async fn new(
        listen_addr: SocketAddr,
        auditor: Auditor,
        storage: Arc<RwLock<Storage>>,
        symphonia_sk: Ed25519SK,
    ) -> Result<Self> {
        let listener = TcpListener::bind(listen_addr).await?;
        let network = new_melnet(&listener, STAKE_NET).await.unwrap();
        log::info!("stakeholder actor created");
        Ok(Stakeholder(Arc::new(Actor::spawn(|mbox| {
            stakeholder_loop(mbox, network, auditor, storage, symphonia_sk)
        }))))
    }
}

/// Main loop for the Stakeholder
async fn stakeholder_loop(
    mut mbox: Mailbox<StakeholderMsg>,
    network: melnet::NetState,
    auditor: Auditor,
    storage: Arc<RwLock<Storage>>,
    symphonia_sk: Ed25519SK,
) {
    let msg_loop = async {
        loop {
            let msg = mbox.recv().await;
            unimplemented!()
        }
    };
    let symphonia_loop = async {
        let (in_send, mut in_recv) = mpsc::unbounded();
        // register symphonia verbs
        network.register_verb("symphonia_msg", move |_, smsg: symphonia::SignedMessage| {
            in_send.unbounded_send(smsg).unwrap();
            Ok(true)
        });
        loop {
            Timer::after(Duration::from_secs(5)).await;
            let proposal = Arc::new(storage.read().curr_state.clone().finalize());
            // create a configuration
            let symphonia_config = symphonia::Config {
                // sender weight is computed by using the stake mapping
                sender_weight: {
                    let proposal = proposal.clone();
                    Arc::new(move |pk: Ed25519PK| {
                        // look up in the stake info
                        proposal
                            .inner_ref()
                            .stakes
                            .vote_power(proposal.inner_ref().height / STAKE_EPOCH, pk)
                    })
                },
                // view_leader right now is hardcoded
                view_leader: {
                    //let proposal = proposal.clone();
                    Arc::new(move |view: u64| insecure_testnet_keygen(0 as usize).0)
                },
                is_valid_prop: {
                    let proposal = proposal.clone();
                    Arc::new(|prop_msg: &[u8]| {
                        log::debug!("is_valid_prop forcing true");
                        true
                    })
                },
                gen_proposal: {
                    let blk = proposal.to_block();
                    Arc::new(move || bincode::serialize(&blk).unwrap())
                },
                my_sk: symphonia_sk,
                my_pk: symphonia_sk.to_public(),
            };
            // run symphonia.
            // first we construct a pacemaker
            // then we simultaneously:
            // - feed incoming messages into the pacemaker
            // - broadcast outgoing messages
            // - wait for a decision
            let pacemaker = {
                let mach = symphonia::Machine::new(symphonia_config);
                symphonia::Pacemaker::new(mach)
            };
            let decision: symphonia::QuorumCert = loop {
                select! {
                    new_msg = in_recv.next() => {
                        pacemaker.process_input(new_msg.unwrap())
                    }
                    outgoing_msg = pacemaker.next_output().fuse() => {
                        let (dest, outgoing_msg) = outgoing_msg;
                        symphonia_multicast(dest, outgoing_msg, network.routes()).await;
                    }
                    decision = pacemaker.decision().fuse() => {
                        break decision
                    }
                }
            };
            // merge in the decision
            let decided_val: blkstructs::Block = bincode::deserialize(&decision.node.prop)
                .expect("decision reached on something that isn't a valid block?!");
            storage
                .write()
                .apply_block(decided_val)
                .expect("decided block wasn't accepted?!");
            auditor.send(AuditorMsg::SendFinalizedBlk(
                storage.read().last_block().unwrap(),
                decision,
            ));
        }
    };
    futures::join!(msg_loop, symphonia_loop);
}

async fn symphonia_multicast(
    dest: Option<Ed25519PK>,
    msg: symphonia::SignedMessage,
    routes: Vec<SocketAddr>,
) {
    for dest in routes {
        let msg = msg.clone();
        smol::spawn(async move {
            let _: bool = melnet::g_client()
                .request(dest, STAKE_NET, "symphonia_msg", msg)
                .await
                .unwrap_or(false);
        })
        .detach();
    }
}

#[derive(Debug)]
pub enum StakeholderMsg {
    Receive,
}
