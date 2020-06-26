use log::trace;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use rlp::{Decodable, Encodable};
use rlp_derive::*;
use std::collections::{BTreeMap, BTreeSet};
use std::convert::TryFrom;
use std::sync::Arc;

const BFT_THRESHOLD: f64 = 0.7;

#[derive(Default)]
struct MachVars {
    curr_view: u64,
    locked_qc: Option<QuorumCert>,
    prepare_qc: Option<QuorumCert>,
    curr_proposal: Option<Node>,

    seen_msgs: BTreeMap<u64, BTreeSet<Message>>,
    output_msgs: Vec<(Option<tmelcrypt::Ed25519PK>, SignedMessage)>,
}

impl MachVars {
    fn msgs_in_view<'a>(&'a mut self, view: u64) -> &'a mut BTreeSet<Message> {
        let new = BTreeSet::new();
        let out = self.seen_msgs.insert(self.curr_view, new);
        if let Some(old) = out {
            self.seen_msgs.insert(view, old);
        }
        self.seen_msgs.get_mut(&view).unwrap()
    }

    fn msgs_in_curr_view<'a>(&'a mut self) -> &'a mut BTreeSet<Message> {
        let v = self.curr_view;
        self.msgs_in_view(v)
    }
}

pub struct Machine {
    globals: MachVars,
    curr_phase: Phase,
    config: Config,
    decision: Option<QuorumCert>,
}

impl Machine {
    // Processes a message.
    pub fn process_input(&mut self, msg: SignedMessage) {
        trace!("[V={}] process_msg({:?})", self.globals.curr_view, msg);
        if let Some(msg) = msg.validate() {
            // first we mutate the state
            {
                // get the vector of messages at the right view #
                let existing = self.globals.msgs_in_view(msg.view_number);
                for m in existing.iter() {
                    if m.phase == msg.phase && m.sender == msg.sender {
                        return;
                    }
                }
                existing.insert(msg);
            }
            // then we react
            self.react();
        }
    }
    // Drain outgoing messages.
    pub fn drain_output(&mut self) -> Vec<(Option<tmelcrypt::Ed25519PK>, SignedMessage)> {
        std::mem::replace(&mut self.globals.output_msgs, Vec::new())
    }

    fn sum_weights(&self, mm: &[Message]) -> f64 {
        mm.iter()
            .map(|m| (self.config.sender_weight)(m.sender))
            .sum()
    }

    fn react(&mut self) {
        let leader_pk = (self.config.view_leader)(self.globals.curr_view);
        let is_leader = leader_pk == self.config.my_pk;
        println!("is_leader = {}, phase = {:?}", is_leader, self.curr_phase);
        loop {
            let curr_view = self.globals.curr_view;
            match self.curr_phase {
                // Prepare phase
                Phase::Prepare => {
                    if is_leader {
                        let high_qc = {
                            if self.globals.curr_view == 0 {
                                None
                            } else {
                                // wait for n-f newview messages
                                let mm: Vec<Message> = self
                                    .globals
                                    .msgs_in_view(self.globals.curr_view - 1)
                                    .iter()
                                    .filter(|msg| msg.phase == Phase::NewView)
                                    .cloned()
                                    .collect();
                                if self.sum_weights(&mm) < BFT_THRESHOLD {
                                    trace!(
                                        "[V={}] Prepare -> is_leader -> not enough votes",
                                        self.globals.curr_view
                                    );
                                    return;
                                }
                                // construct highQC
                                let high_qc = mm
                                    .iter()
                                    .max_by_key(|m| m.justify.as_ref().unwrap().view_number)
                                    .unwrap()
                                    .clone()
                                    .justify
                                    .unwrap();
                                trace!(
                                    "[V={}] Prepare -> is_leader -> enough votes -> high_qc = {:?}",
                                    self.globals.curr_view,
                                    high_qc
                                );
                                Some(high_qc)
                            }
                        };
                        let curr_proposal = Node::create_leaf(
                            match high_qc.clone() {
                                Some(high_qc) => Some(high_qc.node),
                                None => None,
                            },
                            (self.config.gen_proposal)(),
                        );
                        self.globals.curr_proposal = Some(curr_proposal.clone());
                        trace!(
                            "[V={}] Prepare -> is_leader -> enough votes -> curr_proposal = {:?}",
                            self.globals.curr_view,
                            curr_proposal
                        );
                        // broadcast transition into next phase
                        self.broadcast(None, self.make_msg(Phase::Prepare, curr_proposal, high_qc));
                    }
                    // wait for a message from deh leader
                    if let Some(m) = self
                        .globals
                        .msgs_in_curr_view()
                        .clone()
                        .iter()
                        .find_map(|m| {
                            if m.sender == leader_pk
                                && (match &m.justify {
                                    Some(justify) => m.node.parent_hash == justify.node.hash(),
                                    None => curr_view == 0,
                                })
                                && self.is_safe_node(&m.node)
                            {
                                return Some(m);
                            }

                            None
                        })
                    {
                        trace!(
                            "[V={}] Prepare -> !is_leader -> got leader msg  {:?}",
                            self.globals.curr_view,
                            m
                        );
                        self.broadcast(
                            Some(leader_pk),
                            self.make_msg(Phase::Prepare, m.node.clone(), None),
                        );
                        self.curr_phase = Phase::PreCommit;
                    } else {
                        trace!(
                            "[V={}] Prepare -> !is_leader -> no leader msg",
                            self.globals.curr_view
                        );
                        return;
                    }
                }
                // Precommit phase
                Phase::PreCommit => {
                    if is_leader {
                        let curr_proposal = self.globals.curr_proposal.clone();
                        // wait for threshold votes
                        let vv: Vec<Message> = self
                            .globals
                            .msgs_in_curr_view()
                            .iter()
                            .filter(|m| {
                                m.phase == Phase::Prepare && Some(m.node.clone()) == curr_proposal
                            })
                            .cloned()
                            .collect();
                        if self.sum_weights(&vv) < BFT_THRESHOLD {
                            trace!(
                                "[V={}] PreCommit -> is_leader -> not enough votes",
                                self.globals.curr_view
                            );
                            return;
                        }
                        // create QC
                        self.globals.prepare_qc = Some(QuorumCert {
                            phase: Phase::Prepare,
                            view_number: self.globals.curr_view,
                            node: self.globals.curr_proposal.clone().unwrap(),
                            witnesses: vv,
                        });
                        trace!(
                            "[V={}] PreCommit -> is_leader -> prepare_qc created {:?}",
                            self.globals.curr_view,
                            self.globals.prepare_qc
                        );
                        // broadcast
                        self.broadcast(
                            None,
                            self.make_msg(
                                Phase::PreCommit,
                                self.globals.curr_proposal.clone().unwrap(),
                                self.globals.prepare_qc.clone(),
                            ),
                        )
                    }
                    let leader_msg = self.globals.msgs_in_curr_view().iter().cloned().find(|m| {
                        m.sender == leader_pk
                            && m.phase == Phase::Prepare
                            && matching_qc(&m.justify, Phase::Prepare, curr_view)
                    });
                    trace!(
                        "[V={}] PreCommit -> !is_leader -> leader_msg = {:?}",
                        self.globals.curr_view,
                        leader_msg
                    );
                    if let Some(msg) = leader_msg {
                        self.globals.prepare_qc = msg.justify.clone();
                        self.broadcast(
                            Some(leader_pk),
                            self.make_msg(Phase::PreCommit, msg.justify.unwrap().node, None),
                        );
                        self.curr_phase = Phase::Commit
                    } else {
                        return;
                    }
                }
                // Commit phase
                Phase::Commit => {
                    if is_leader {
                        let curr_proposal = self.globals.curr_proposal.clone();
                        // wait for enough votes
                        let vv: Vec<Message> = self
                            .globals
                            .msgs_in_curr_view()
                            .iter()
                            .filter(|m| {
                                m.phase == Phase::PreCommit && Some(m.node.clone()) == curr_proposal
                            })
                            .cloned()
                            .collect();
                        if self.sum_weights(&vv) < BFT_THRESHOLD {
                            trace!(
                                "[V={}] Commit -> is_leader -> not enough votes",
                                self.globals.curr_view
                            );
                            return;
                        }
                        // build QC
                        let precommit_qc = QuorumCert {
                            phase: Phase::PreCommit,
                            node: curr_proposal.unwrap(),
                            view_number: self.globals.curr_view,
                            witnesses: vv,
                        };
                        trace!(
                            "[V={}] Commit -> is_leader -> precommit_qc = {:?}",
                            self.globals.curr_view,
                            precommit_qc
                        );
                        self.broadcast(
                            None,
                            self.make_msg(
                                Phase::Commit,
                                self.globals.curr_proposal.clone().unwrap(),
                                Some(precommit_qc),
                            ),
                        )
                    }
                    let view = self.globals.curr_view;
                    // wait for leader to talk
                    let leader_msg = self.globals.msgs_in_curr_view().iter().cloned().find(|m| {
                        m.sender == leader_pk
                            && m.phase == Phase::Commit
                            && m.justify.is_some()
                            && m.justify.as_ref().unwrap().phase == Phase::PreCommit
                            && m.justify.as_ref().unwrap().view_number == view
                    });
                    if let Some(leader_msg) = leader_msg {
                        self.globals.locked_qc = leader_msg.justify.clone();
                        trace!(
                            "[V={}] Commit -> !is_leader -> locked_qc = {:?}",
                            self.globals.curr_view,
                            self.globals.locked_qc
                        );
                        // send the vote back to the leader
                        self.broadcast(
                            Some(leader_pk),
                            self.make_msg(Phase::Commit, leader_msg.justify.unwrap().node, None),
                        );
                        self.curr_phase = Phase::Decide
                    } else {
                        return;
                    }
                } // Decide phase
                Phase::Decide => {
                    if is_leader {
                        let curr_proposal = self.globals.curr_proposal.clone();
                        let vv: Vec<Message> = self
                            .globals
                            .msgs_in_curr_view()
                            .iter()
                            .filter(|m| {
                                m.phase == Phase::Commit && Some(m.node.clone()) == curr_proposal
                            })
                            .cloned()
                            .collect();
                        if self.sum_weights(&vv) < BFT_THRESHOLD {
                            trace!(
                                "[V={}] PreCommit -> is_leader -> not enough votes",
                                self.globals.curr_view
                            );
                            return;
                        }
                        let commit_qc = QuorumCert {
                            phase: Phase::Commit,
                            node: curr_proposal.unwrap(),
                            view_number: self.globals.curr_view,
                            witnesses: vv,
                        };
                        self.broadcast(
                            None,
                            self.make_msg(
                                Phase::Decide,
                                self.globals.curr_proposal.clone().unwrap(),
                                Some(commit_qc),
                            ),
                        )
                    }
                    // wait for message
                    let view = self.globals.curr_view;
                    let leader_msg = self.globals.msgs_in_curr_view().iter().cloned().find(|m| {
                        m.sender == leader_pk
                            && m.phase == Phase::Decide
                            && m.justify.is_some()
                            && m.justify.as_ref().unwrap().phase == Phase::Commit
                            && m.justify.as_ref().unwrap().view_number == view
                    });
                    if let Some(m) = leader_msg {
                        self.decision = m.justify
                    }
                    return;
                }
                Phase::NewView => unimplemented!("WAT"),
            }
        }
    }

    fn make_msg(&self, kind: Phase, node: Node, qc: Option<QuorumCert>) -> Message {
        Message {
            phase: kind,
            node,
            justify: qc,
            view_number: self.globals.curr_view,
            sender: self.config.my_pk,
        }
    }

    fn broadcast(&mut self, dest: Option<tmelcrypt::Ed25519PK>, msg: Message) {
        trace!("broadcast {:?}", msg);
        self.globals
            .output_msgs
            .push((dest, msg.clone().sign(self.config.my_sk)));
        if dest == None || dest == Some(self.config.my_pk) {
            self.globals.msgs_in_view(msg.view_number).insert(msg);
        }
    }

    fn is_safe_node(&self, node: &Node) -> bool {
        // TODO
        true
    }

    // main thread
    pub fn new(config: Config) -> Self {
        let mut m = Machine {
            globals: MachVars::default(),
            config,
            curr_phase: Phase::Prepare,
            decision: None,
        };
        m.react();
        m
    }
}

fn matching_msg(msg: &Message, phase: Phase, view: u64) -> bool {
    msg.phase == phase && msg.view_number == view
}

fn matching_qc(qc: &Option<QuorumCert>, phase: Phase, view: u64) -> bool {
    match qc {
        None => view == 0 && (phase == Phase::Prepare),
        Some(qc) => qc.phase == phase && qc.view_number == view,
    }
}

#[derive(Clone)]
pub struct Config {
    pub sender_weight: Arc<dyn Fn(tmelcrypt::Ed25519PK) -> f64>,
    pub view_leader: Arc<dyn Fn(u64) -> tmelcrypt::Ed25519PK>,
    pub is_valid_prop: Arc<dyn Fn(Vec<u8>) -> bool>,
    pub gen_proposal: Arc<dyn Fn() -> Vec<u8>>,
    pub my_sk: tmelcrypt::Ed25519SK,
    pub my_pk: tmelcrypt::Ed25519PK,
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, RlpEncodable, RlpDecodable)]
pub struct Message {
    phase: Phase,
    node: Node,
    justify: Option<QuorumCert>,
    sender: tmelcrypt::Ed25519PK,
    view_number: u64,
}

impl Message {
    pub fn sign(self, sk: tmelcrypt::Ed25519SK) -> SignedMessage {
        let msg_bts = rlp::encode(&self);
        let sig = sk.sign(&msg_bts);
        SignedMessage {
            msg: self,
            signature: sig,
        }
    }
}

#[derive(RlpEncodable, RlpDecodable, Clone, Debug)]
pub struct SignedMessage {
    msg: Message,
    signature: Vec<u8>,
}

impl SignedMessage {
    pub fn validate(self) -> Option<Message> {
        let msg_bts = rlp::encode(&self.msg);
        if self.msg.sender.verify(&msg_bts, &self.signature) {
            Some(self.msg)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Copy, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum Phase {
    NewView = 0x01,
    Prepare = 0x02,
    PreCommit = 0x03,
    Commit = 0x04,
    Decide = 0x05,
}

impl Encodable for Phase {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        (*self as u8).rlp_append(s)
    }
}

impl Decodable for Phase {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let raw = u8::decode(rlp)?;
        if let Ok(x) = Phase::try_from(raw) {
            Ok(x)
        } else {
            Err(rlp::DecoderError::Custom("bad phase"))
        }
    }
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, RlpEncodable, RlpDecodable)]
pub struct QuorumCert {
    phase: Phase,
    view_number: u64,
    node: Node,
    witnesses: Vec<Message>,
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, RlpEncodable, RlpDecodable)]
pub struct Node {
    parent_hash: tmelcrypt::HashVal,
    prop: Vec<u8>,
}

impl Node {
    pub fn create_leaf(parent: Option<Node>, prop: Vec<u8>) -> Self {
        Node {
            parent_hash: match parent {
                None => tmelcrypt::HashVal([0; 32]),
                Some(p) => p.hash(),
            },
            prop,
        }
    }
    pub fn hash(&self) -> tmelcrypt::HashVal {
        tmelcrypt::hash_single(&rlp::encode(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_party() {
        env_logger::init();
        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let config = Config {
            sender_weight: Arc::new(|pk| 1.0),
            view_leader: Arc::new(move |_| pk),
            is_valid_prop: Arc::new(|_| true),
            gen_proposal: Arc::new(|| b"Hello World".to_vec()),
            my_sk: sk,
            my_pk: pk,
        };
        let machine = Machine::new(config);
    }
}
