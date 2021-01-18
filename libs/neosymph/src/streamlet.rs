use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};

use anyhow::Context;
use async_trait::async_trait;
use blkstructs::{Block, SealedState, StakeMapping, Transaction};
use broadcaster::BroadcastChannel;
use chainstate::ChainState;
use msg::{ActualProposal, Message, ProposalMsg, SignedMessage, Signer};
use smol::channel::{Receiver, Sender};
use smol_timeout::TimeoutExt;
use tmelcrypt::{Ed25519PK, Ed25519SK, HashVal};

use crate::msg::{self, VoteMsg};

mod chainstate;

const BLOCK_INTERVAL_SECS: u64 = 30;

/// A Streamlet is a single-epoch  instance of Symphonia.
pub struct Streamlet<N: Network, L: TxLookup> {
    chain: ChainState,
    partial_props: HashMap<HashVal, ProposalMsg>,

    signer: Signer,
    cfg: StreamletCfg<N, L>,

    send_event: Option<Sender<(StreamletEvt, ChainState)>>,

    last_lnc: HashVal,
}

impl<N: Network, L: TxLookup> Streamlet<N, L> {
    /// Create a new pacemaker.
    pub fn new(cfg: StreamletCfg<N, L>) -> Self {
        Self {
            chain: ChainState::new(cfg.genesis.clone(), cfg.stakes.clone(), cfg.epoch),
            partial_props: HashMap::new(),

            signer: Signer::new(cfg.my_sk),
            cfg,

            send_event: None,

            last_lnc: tmelcrypt::hash_single(b"couldn't be this"),
        }
    }

    /// Subscribe to events.
    pub fn subscribe(&mut self) -> Receiver<(StreamletEvt, ChainState)> {
        let (send, recv) = smol::channel::unbounded();
        self.send_event = Some(send);
        recv
    }

    /// Runs the pacemaker indefinitely.
    pub async fn run(mut self) {
        let my_public = self.cfg.my_sk.to_public();
        self.notify_lnc();
        loop {
            let height = self.wait_block().await;
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

            // process received messages "simultaneously" with processing partial proposals.
            for _ in 0..BLOCK_INTERVAL_SECS * 10 / 2 {
                // give the partial proposals a chance at least every second
                self.promote_partials().await;
                let msg = self
                    .cfg
                    .network
                    .receive()
                    .timeout(Duration::from_millis(100))
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
            }
        }
    }

    fn process_msg(&mut self, sender: Ed25519PK, msg: Message) -> anyhow::Result<()> {
        match msg {
            Message::Proposal(prop) => {
                log::warn!("STUPIDLY putting in proposal");
                self.partial_props.insert(prop.proposal.header.hash(), prop);
            }
            Message::Vote(vmsg) => self
                .chain
                .process_vote(sender, vmsg.voting_for)
                .context("can't process vote")?,
            _ => anyhow::bail!("not a valid message in this context"),
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
            let _ = send.try_send((evt, self.chain.clone()));
        }
    }

    /// waits until the next block height, then returns that height
    async fn wait_block(&self) -> u64 {
        let now = SystemTime::now();
        let elapsed_secs = now
            .duration_since(self.cfg.start_time)
            .expect("clock randomly jumped, that breaks streamlet")
            .as_secs();
        let next_height = elapsed_secs / BLOCK_INTERVAL_SECS + 1;
        let next_time =
            self.cfg.start_time + Duration::from_secs(next_height * BLOCK_INTERVAL_SECS);
        while SystemTime::now() < next_time {
            smol::Timer::after(Duration::from_millis(100)).await;
        }
        next_height
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

    pub my_sk: Ed25519SK,

    pub get_proposer: Box<dyn Fn(u64) -> Ed25519PK + Send + Sync>,
}

impl<N: Network, L: TxLookup> StreamletCfg<N, L> {
    fn get_proposer(&self, height: u64) -> Ed25519PK {
        (self.get_proposer)(height)
    }
}

/// An async-trait that represents a network.
#[async_trait]
pub trait Network {
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
pub trait TxLookup {
    /// Look up a transaction by its hash
    fn lookup(&self, hash: HashVal) -> Option<Transaction>;
}
