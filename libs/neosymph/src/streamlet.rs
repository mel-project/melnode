use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use crate::msg::{self, VoteMsg};
use anyhow::Context;
use async_trait::async_trait;
use blkstructs::{Block, SealedState, StakeMapping, Transaction, STAKE_EPOCH};
use broadcaster::BroadcastChannel;
use chainstate::ChainState;
use msg::{ActualProposal, Message, ProposalMsg, SignedMessage, Signer};
use smol::channel::{Receiver, Sender};
use smol::prelude::*;
use smol_timeout::TimeoutExt;
use tmelcrypt::{Ed25519PK, Ed25519SK, HashVal};

mod chainstate;
mod imtree;

/// A Streamlet is a single-epoch  instance of Symphonia.
pub struct Streamlet<N: Network, L: TxLookup> {
    chain: ChainState,
    partial_props: HashMap<HashVal, ProposalMsg>,

    signer: Signer,
    cfg: StreamletCfg<N, L>,

    send_event: Option<Sender<StreamletEvt>>,

    interval: Duration,
    last_lnc: HashVal,
}

impl<N: Network, L: TxLookup> Streamlet<N, L> {
    /// Create a new pacemaker.
    pub fn new(cfg: StreamletCfg<N, L>) -> Self {
        let interval = cfg.interval;
        Self {
            chain: ChainState::new(cfg.genesis.clone(), cfg.stakes.clone(), cfg.epoch),
            partial_props: HashMap::new(),

            signer: Signer::new(cfg.my_sk),
            cfg,
            send_event: None,

            interval,
            last_lnc: tmelcrypt::hash_single(b"couldn't be this"),
        }
    }

    /// Starts the streamlet pacemaker, returning a handle with which to interact with the pacemaker.
    pub fn start(mut self) -> StreamletHandle {
        let (send_event, recv_event) = smol::channel::unbounded();
        let (send_force_finalize, recv_force_finalize) = smol::channel::unbounded();
        self.send_event = Some(send_event);
        let exec: smol::Executor<'static> = smol::Executor::new();
        exec.spawn(async move {
            let my_public = self.cfg.my_sk.to_public();
            loop {
                self.notify_lnc();
                let (height, height_time) = self.next_height_time();

                // process received messages
                let process_loop = async {
                    loop {
                        self.promote_partials().await;
                        let msg = self
                            .cfg
                            .network
                            .receive()
                            .timeout(Duration::from_millis(500))
                            .await;
                        if let Some(msg) = msg {
                            let sender = msg.sender;
                            let msg_body = msg.body().cloned();
                            if let Some(msg_body) = msg_body {
                                if let Err(err) = self.process_msg(sender, msg_body) {
                                    log::warn!("can't process msg: {:?}", err);
                                }
                                self.notify_lnc()
                            }
                        }
                        if let Ok(val) = recv_force_finalize.try_recv() {
                            self.partial_props.clear();
                            self.chain.force_finalize(val);
                        }
                    }
                };
                process_loop
                    .race(async {
                        while SystemTime::now() < height_time {
                            smol::Timer::after(Duration::from_millis(100)).await;
                        }
                    })
                    .await;

                let proposer = self.cfg.get_proposer(height);
                let iam_proposer = proposer == my_public;
                log::debug!(
                    "Reached height {}, proposer is {:?} (iam_proposer = {})",
                    height,
                    proposer,
                    iam_proposer
                );

                // if I'm a proposer, broadcast a proposal
                if iam_proposer {
                    let lnc_tip = self
                        .chain
                        .get_block(self.chain.get_lnc_tip())
                        .expect("must have LNC")
                        .state
                        .clone();
                    if let Some(proposal_content) = self.solicit_proposal(lnc_tip, height).await {
                        log::debug!(
                            "height {}, sending proposal w/ {} txx",
                            height,
                            proposal_content.proposal.txhashes.len()
                        );
                        let msg = self.signer.sign(Message::Proposal(proposal_content));
                        self.cfg.network.broadcast(msg).await;
                    } else {
                        log::warn!("proposal event wasn't responded to correctly")
                    }
                }
            }
        })
        .detach();
        StreamletHandle {
            recv_event,
            send_force_finalize,
            exec,
        }
    }

    fn process_msg(&mut self, sender: Ed25519PK, msg: Message) -> anyhow::Result<()> {
        match msg {
            Message::Proposal(prop) => {
                log::warn!("STUPIDLY putting in proposal: {:#?}", prop);
                self.partial_props.insert(prop.proposal.header.hash(), prop);
            }
            Message::Vote(vmsg) => self
                .chain
                .process_vote(sender, vmsg.voting_for)
                .context("can't process vote")?,
        }
        Ok(())
    }

    /// promote partial proposals that aren't actually partial anymore
    async fn promote_partials(&mut self) {
        let mut to_move = vec![];
        for (phash, partial) in self.partial_props.iter() {
            let mut actual_txx = vec![];
            for txhash in partial.proposal.txhashes.iter() {
                if let Some(val) = self.cfg.lookup.lookup(*txhash) {
                    actual_txx.push(val);
                }
            }
            log::debug!(
                "trying to promote partial {:?}: {}/{}",
                phash,
                actual_txx.len(),
                partial.proposal.txhashes.len()
            );
            if actual_txx.len() == partial.proposal.txhashes.len() {
                to_move.push(ActualProposal {
                    block: Block {
                        header: partial.proposal.header,
                        transactions: actual_txx.into(),
                        proposer_action: partial.proposal.proposer_action,
                    },
                    last_nonempty: partial.last_nonempty,
                });
            }
        }
        if !to_move.is_empty() {
            log::debug!("promoting {} proposals", to_move.len());
        }
        for prop in to_move {
            self.partial_props.remove(&prop.block.header.hash());
            let height = prop.height();
            let hash = prop.block.header.hash();
            if let Err(err) = self.chain.process_proposal(prop) {
                log::warn!("rejecting proposal at height {}: {:?}", height, err);
            } else {
                log::debug!(
                    "{:?} voting for proposal at height {}",
                    self.cfg.my_sk.to_public(),
                    height
                );
                let msg = self
                    .signer
                    .sign(Message::Vote(VoteMsg { voting_for: hash }));
                self.cfg.network.broadcast(msg).await;
            }
        }
    }

    /// notifies the LNC
    fn notify_lnc(&mut self) {
        let lnc_tip_hash = self.chain.get_lnc_tip();
        if self.last_lnc != lnc_tip_hash {
            self.last_lnc = lnc_tip_hash;
            log::debug!("notifying new LNC: {:?}", lnc_tip_hash);
            self.event(StreamletEvt::LastNotarizedTip(
                self.chain.get_block(lnc_tip_hash).unwrap().state.clone(),
            ))
        }
        // drain
        let drained = self.chain.drain_finalized();
        if !drained.is_empty() {
            self.event(StreamletEvt::Finalize(drained));
        }
    }

    /// solicit a proposal
    async fn solicit_proposal(&mut self, lnc_tip: SealedState, height: u64) -> Option<ProposalMsg> {
        let (send, recv) = async_oneshot::oneshot();
        let evt = StreamletEvt::SolicitProp(lnc_tip, height, send);
        self.event(evt);
        recv.await.ok()
    }

    /// emits an event
    fn event(&mut self, evt: StreamletEvt) {
        if let Some(send) = self.send_event.as_ref() {
            let _ = send.try_send(evt);
        }
    }

    /// waits until the next block height, then returns that height
    fn next_height_time(&self) -> (u64, SystemTime) {
        let now = SystemTime::now();
        let elapsed_time = now
            .duration_since(self.cfg.start_time)
            .expect("clock randomly jumped, that breaks streamlet");
        let next_height = elapsed_time.as_millis() / self.interval.as_millis();
        let next_height = next_height as u64;
        let next_time = self.cfg.start_time + self.interval * (next_height as u32 + 1);
        (next_height, next_time)
    }
}

/// A handle with which to interact with a running Streamlet instance.
pub struct StreamletHandle {
    recv_event: Receiver<StreamletEvt>,
    send_force_finalize: Sender<SealedState>,
    exec: smol::Executor<'static>,
}

impl StreamletHandle {
    /// Waits for the next event.
    pub async fn next_event(&mut self) -> StreamletEvt {
        self.exec
            .run(self.recv_event.recv())
            .await
            .expect("background streamlet task somehow died")
    }

    /// Forcibly finalizes a block, pruning the blockchain.
    pub fn force_finalize(&mut self, state: SealedState) {
        self.send_force_finalize
            .try_send(state)
            .expect("background streamlet somehow couldn't accept a force finalize")
    }
}

/// An event happening in streamlet
#[derive(Debug)]
pub enum StreamletEvt {
    SolicitProp(SealedState, u64, async_oneshot::Sender<ProposalMsg>),
    LastNotarizedTip(SealedState),
    Finalize(Vec<SealedState>),
}

/// Configuration for a pacemaker.
pub struct StreamletCfg<N: Network, L: TxLookup> {
    pub network: N,
    pub lookup: L,
    pub genesis: SealedState,
    pub stakes: StakeMapping,
    pub epoch: u64,
    pub start_time: SystemTime,
    pub interval: Duration,

    pub my_sk: Ed25519SK,

    pub get_proposer: Box<dyn Fn(u64) -> Ed25519PK + Send + Sync>,
}

impl<N: Network, L: TxLookup> StreamletCfg<N, L> {
    /// Creates a streamlet configuration given a state, network, and lookup function.
    pub fn new(
        last_sealed: SealedState,
        my_sk: Ed25519SK,
        network: N,
        lookup: L,
        interval: Duration,
    ) -> Self {
        let first_stake = last_sealed.inner_ref().stakes.val_iter().next().unwrap();
        Self {
            network,
            lookup,
            genesis: last_sealed.clone(),
            stakes: last_sealed.inner_ref().stakes.clone(),
            epoch: last_sealed.inner_ref().height / STAKE_EPOCH,
            start_time: std::time::UNIX_EPOCH + Duration::from_secs(1614578400),
            interval,
            my_sk,
            get_proposer: Box::new(move |_height| first_stake.pubkey),
        }
    }

    fn get_proposer(&self, height: u64) -> Ed25519PK {
        (self.get_proposer)(height)
    }
}

/// An async-trait that represents a network.
#[async_trait]
pub trait Network: Send + Sync + 'static {
    /// Broadcasts a message to the network.
    async fn broadcast(&self, msg: SignedMessage);

    /// Waits for the next message from the network.
    async fn receive(&mut self) -> SignedMessage;
}

/// A mock network based on channels.
#[derive(Clone)]
pub struct MockNet {
    bus: BroadcastChannel<SignedMessage>,
}

impl Default for MockNet {
    fn default() -> Self {
        Self::new()
    }
}

impl MockNet {
    pub fn new() -> Self {
        Self {
            bus: BroadcastChannel::new(),
        }
    }
}

#[async_trait]
impl Network for MockNet {
    async fn broadcast(&self, msg: SignedMessage) {
        self.bus.send(&msg).await.expect("this can't be closed");
    }

    async fn receive(&mut self) -> SignedMessage {
        self.bus.recv().await.expect("can't be closed")
    }
}

/// A trait that represents a backend for looking up transactions by hash
pub trait TxLookup: Send + Sync + 'static {
    /// Look up a transaction by its hash
    fn lookup(&self, hash: HashVal) -> Option<Transaction>;
}
