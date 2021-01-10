use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};

use anyhow::Chain;
use async_trait::async_trait;
use blkstructs::{FinalizedState, StakeMapping, Transaction};
use chainstate::ChainState;
use msg::{Message, ProposalMsg, SignedMessage, Signer};
use tmelcrypt::{Ed25519PK, Ed25519SK, HashVal};

use crate::msg;

mod chainstate;

/// A Streamlet is a single-epoch  instance of Symphonia.
pub struct Streamlet<N: Network, L: TxLookup> {
    chain: ChainState,
    partial_props: HashMap<HashVal, ProposalMsg>,

    signer: Signer,
    cfg: StreamletCfg<N, L>,
}

impl<N: Network, L: TxLookup> Streamlet<N, L> {
    /// Create a new pacemaker.
    pub fn new(cfg: StreamletCfg<N, L>) -> Self {
        Self {
            chain: ChainState::new(cfg.genesis.clone(), cfg.stakes.clone(), cfg.epoch),
            partial_props: HashMap::new(),

            signer: Signer::new(cfg.my_sk),
            cfg,
        }
    }

    /// Runs the pacemaker until some block has finalized.
    pub async fn run_till_final(&mut self) -> FinalizedState {
        let my_public = self.cfg.my_sk.to_public();
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
                let proposal_content = self.cfg.gen_proposal(height, lnc_tip);
                log::debug!(
                    "height {}, sending proposal w/ {} txx",
                    height,
                    proposal_content.proposal.txhashes.len()
                );
                let msg = self.signer.sign(Message::Proposal(proposal_content));
                self.cfg.network.broadcast(msg);
            }

            // process received messages
            loop {
                let msg = self.cfg.network.receive().await;
                let msg_body = msg.body();
                log::debug!("got message {:?}", msg);
            }
        }
    }

    /// waits until the next block height, then returns that height
    async fn wait_block(&self) -> u64 {
        let now = SystemTime::now();
        let elapsed_secs = now
            .duration_since(self.cfg.start_time)
            .expect("clock randomly jumped, that breaks streamlet")
            .as_secs();
        let next_height = elapsed_secs / 30 + 1;
        let next_time = self.cfg.start_time + Duration::from_secs(next_height * 30);
        while SystemTime::now() < next_time {
            smol::Timer::after(Duration::from_millis(100)).await;
        }
        next_height
    }
}

/// Configuration for a pacemaker.
pub struct StreamletCfg<N: Network, L: TxLookup> {
    network: N,
    lookup: L,
    genesis: FinalizedState,
    stakes: StakeMapping,
    epoch: u64,
    start_time: SystemTime,

    my_sk: Ed25519SK,

    gen_proposal: Box<dyn FnMut(u64, FinalizedState) -> ProposalMsg + Send>,
    get_proposer: Box<dyn Fn(u64) -> Ed25519PK + Send>,
}

impl<N: Network, L: TxLookup> StreamletCfg<N, L> {
    fn get_proposer(&self, height: u64) -> Ed25519PK {
        (self.get_proposer)(height)
    }

    fn gen_proposal(&mut self, height: u64, lnc_tip: FinalizedState) -> ProposalMsg {
        (self.gen_proposal)(height, lnc_tip)
    }
}

/// An async-trait that represents a network.
#[async_trait]
pub trait Network {
    /// Broadcasts a message to the network.
    fn broadcast(&self, msg: SignedMessage);

    /// Waits for the next message from the network.
    async fn receive(&self) -> SignedMessage;
}

/// A mock network based on channels.
pub struct MockNet {
    sender: Arc<async_watch2::Sender<SignedMessage>>,
    receiver: smol::lock::Mutex<async_watch2::Receiver<SignedMessage>>,
    receiver_copy: async_watch2::Receiver<SignedMessage>,
}

impl Clone for MockNet {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            receiver: smol::lock::Mutex::new(self.receiver_copy.clone()),
            receiver_copy: self.receiver_copy.clone(),
        }
    }
}

#[async_trait]
impl Network for MockNet {
    fn broadcast(&self, msg: SignedMessage) {
        self.sender.broadcast(msg).expect("this can't be closed");
    }

    async fn receive(&self) -> SignedMessage {
        self.receiver
            .lock()
            .await
            .recv()
            .await
            .expect("can't be closed")
    }
}

/// A trait that represents a backend for looking up transactions by hash
pub trait TxLookup {
    /// Look up a transaction by its hash
    fn lookup(hash: HashVal) -> Option<Transaction>;
}
