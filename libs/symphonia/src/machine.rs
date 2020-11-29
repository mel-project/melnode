use crate::common::*;
use log::trace;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

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
    fn msgs_in_view(&mut self, view: u64) -> &mut BTreeSet<Message> {
        let new = BTreeSet::new();
        let out = self.seen_msgs.insert(view, new);
        if let Some(old) = out {
            self.seen_msgs.insert(view, old);
        }
        self.seen_msgs.get_mut(&view).unwrap()
    }

    fn msgs_in_curr_view(&mut self) -> &mut BTreeSet<Message> {
        let v = self.curr_view;
        self.msgs_in_view(v)
    }
}

/// Machine implements a HotStuff-like central state machine.
pub struct Machine {
    globals: MachVars,
    curr_phase: Phase,
    config: Config,
    decision: Option<QuorumCert>,
}

impl Machine {
    // Processes a message.
    pub fn process_input(&mut self, msg: SignedMessage) {
        if let Some(msg) = msg.validate() {
            // if this message not from a voter, or too far into the future, die
            if (self.config.sender_weight)(msg.sender) == 0.0
                || msg.view_number > self.globals.curr_view + 10
            {
                return;
            }
            // first we mutate the state
            {
                // get the vector of messages at the right view #
                let existing = self.globals.msgs_in_view(msg.view_number);
                for m in existing.iter() {
                    if m.phase == msg.phase && m.sender == msg.sender {
                        return;
                    }
                }
                if let Some(justify) = &msg.justify {
                    if self.config.qc_tally(justify) < BFT_THRESHOLD {
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
    // Decision
    pub fn decision(&self) -> Option<QuorumCert> {
        self.decision.clone()
    }
    // New view
    pub fn new_view(&mut self) {
        self.curr_phase = Phase::NewView;
        self.react();
    }

    fn sum_weights(&self, mm: &[Message]) -> f64 {
        mm.iter()
            .map(|m| (self.config.sender_weight)(m.sender))
            .sum()
    }

    fn gather_votes(&mut self, view: u64, phase: Phase) -> Vec<Message> {
        self.globals
            .msgs_in_view(view)
            .iter()
            .filter(|msg| msg.phase == phase && msg.validate_vote())
            .cloned()
            .collect()
    }

    fn react(&mut self) {
        let leader_pk = (self.config.view_leader)(self.globals.curr_view);
        let is_leader = leader_pk == self.config.my_pk;
        loop {
            let curr_view = self.globals.curr_view;
            match self.curr_phase {
                // Prepare phase
                Phase::Prepare => {
                    assert!(curr_view > 0);
                    if is_leader {
                        let high_qc = {
                            // wait for n-f newview messages
                            let mm: Vec<Message> =
                                self.gather_votes(self.globals.curr_view - 1, Phase::NewView);
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
                                .max_by_key(|m| match &m.justify {
                                    Some(justify) => justify.view_number,
                                    None => 0,
                                })
                                .unwrap()
                                .clone()
                                .justify;
                            trace!(
                                "[V={}] Prepare -> is_leader -> enough votes -> high_qc = {:?}",
                                self.globals.curr_view,
                                high_qc
                            );
                            high_qc
                        };
                        let curr_proposal = Node::create_leaf(
                            match &high_qc {
                                Some(high_qc) => Some(&high_qc.node),
                                None => None,
                            },
                            (self.config.gen_proposal)(),
                        );
                        self.globals.curr_proposal = Some(curr_proposal.clone());
                        trace!(
                            "[V={}] Prepare -> is_leader -> enough votes",
                            self.globals.curr_view
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
                                    None => m.node.parent_hash == tmelcrypt::HashVal::default(),
                                })
                                && self.is_safe_node(&m.node, m.justify.as_ref())
                            {
                                return Some(m);
                            }
                            None
                        })
                    {
                        trace!(
                            "[V={}] Prepare -> !is_leader -> got leader msg ",
                            self.globals.curr_view,
                        );
                        self.broadcast(
                            Some(leader_pk),
                            self.make_vote_msg(Phase::Prepare, m.node.clone(), None),
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
                            .gather_votes(self.globals.curr_view, Phase::Prepare)
                            .into_iter()
                            .filter(|m| Some(m.node.clone()) == curr_proposal)
                            .collect();
                        if self.sum_weights(&vv) < BFT_THRESHOLD {
                            trace!(
                                "[V={}] PreCommit -> is_leader -> not enough votes",
                                self.globals.curr_view
                            );
                            return;
                        }
                        // create QC
                        self.globals.prepare_qc = Some(QuorumCert::new(&vv));
                        trace!(
                            "[V={}] PreCommit -> is_leader -> prepare_qc created",
                            self.globals.curr_view
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
                            && m.phase == Phase::PreCommit
                            && matching_qc(&m.justify, Phase::Prepare, curr_view)
                    });
                    trace!(
                        "[V={}] PreCommit -> !is_leader -> leader_msg = {:?}",
                        self.globals.curr_view,
                        leader_msg.is_some()
                    );
                    if let Some(msg) = leader_msg {
                        self.globals.prepare_qc = msg.justify.clone();
                        self.broadcast(
                            Some(leader_pk),
                            self.make_vote_msg(Phase::PreCommit, msg.justify.unwrap().node, None),
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
                            .gather_votes(self.globals.curr_view, Phase::PreCommit)
                            .into_iter()
                            .filter(|m| Some(m.node.clone()) == curr_proposal)
                            .collect();
                        if self.sum_weights(&vv) < BFT_THRESHOLD {
                            trace!(
                                "[V={}] Commit -> is_leader -> not enough votes",
                                self.globals.curr_view
                            );
                            return;
                        }
                        // build QC
                        let precommit_qc = QuorumCert::new(&vv);
                        trace!(
                            "[V={}] Commit -> is_leader -> precommit_qc",
                            self.globals.curr_view
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
                            "[V={}] Commit -> !is_leader -> locked_qc = {}",
                            self.globals.curr_view,
                            self.globals.locked_qc.is_some()
                        );
                        // send the vote back to the leader
                        self.broadcast(
                            Some(leader_pk),
                            self.make_vote_msg(
                                Phase::Commit,
                                leader_msg.justify.unwrap().node,
                                None,
                            ),
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
                            .gather_votes(self.globals.curr_view, Phase::Commit)
                            .into_iter()
                            .filter(|m| Some(m.node.clone()) == curr_proposal)
                            .collect();
                        if self.sum_weights(&vv) < BFT_THRESHOLD {
                            trace!(
                                "[V={}] PreCommit -> is_leader -> not enough votes",
                                self.globals.curr_view
                            );
                            return;
                        }
                        let commit_qc = QuorumCert::new(&vv);
                        trace!(
                            "[V={}] PreCommit -> is_leader -> broadcasting decide with commit_qc",
                            self.globals.curr_view
                        );
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
                    let leader_msg = self.globals.msgs_in_curr_view().iter().cloned().find(|m| {
                        m.sender == leader_pk
                            && m.phase == Phase::Decide
                            && m.justify.is_some()
                            && m.justify.as_ref().unwrap().phase == Phase::Commit
                            && m.justify.as_ref().unwrap().view_number == curr_view
                    });
                    if let Some(m) = leader_msg {
                        trace!(
                            "[V={}] DECIDE QC({:?}, {:?}, {:?})",
                            curr_view,
                            m.justify.as_ref().unwrap().phase,
                            m.justify.as_ref().unwrap().view_number,
                            m.justify.as_ref().unwrap().node.hash()
                        );
                        self.decision = m.justify
                    }
                    return;
                }
                Phase::NewView => {
                    let next_leader = (self.config.view_leader)(self.globals.curr_view + 1);
                    self.broadcast(
                        Some(next_leader),
                        self.make_vote_msg(
                            Phase::NewView,
                            Node::default(),
                            self.globals.prepare_qc.clone(),
                        ),
                    );
                    self.globals.curr_view += 1;
                    self.curr_phase = Phase::Prepare;
                }
            }
        }
    }

    fn make_msg(&self, phase: Phase, node: Node, justify: Option<QuorumCert>) -> Message {
        Message {
            phase,
            node,
            justify,
            view_number: self.globals.curr_view,
            sender: self.config.my_pk,
            partial_sig: None,
        }
    }

    fn make_vote_msg(&self, phase: Phase, node: Node, justify: Option<QuorumCert>) -> Message {
        let sk = self.config.my_sk;
        Message {
            phase,
            node: node.clone(),
            justify,
            view_number: self.globals.curr_view,
            sender: self.config.my_pk,
            partial_sig: Some(
                sk.sign(
                    &bincode::serialize(&PVN {
                        phase,
                        view_number: self.globals.curr_view,
                        node,
                    })
                    .unwrap(),
                ),
            ),
        }
    }

    fn broadcast(&mut self, dest: Option<tmelcrypt::Ed25519PK>, msg: Message) {
        //trace!("broadcast {:?}", msg);
        self.globals
            .output_msgs
            .push((dest, msg.clone().sign(self.config.my_sk)));
        if dest == None || dest == Some(self.config.my_pk) {
            self.globals.msgs_in_view(msg.view_number).insert(msg);
        }
    }

    fn is_safe_node(&self, node: &Node, qc: Option<&QuorumCert>) -> bool {
        let safetylive = match &self.globals.locked_qc {
            Some(lqc) => {
                node.parent_hash == lqc.node.hash()
                    || (match qc {
                        Some(qc) => {
                            trace!(
                                "safe_node? qc has view {:?} but lqc has {:?}",
                                qc.view_number,
                                lqc.view_number
                            );
                            qc.view_number > lqc.view_number
                        }
                        _ => true,
                    })
            }
            None => true,
        };
        safetylive && (self.config.is_valid_prop)(&node.prop)
    }

    // main thread
    pub fn new(config: Config) -> Self {
        let mut m = Machine {
            globals: MachVars::default(),
            config,
            curr_phase: Phase::Prepare,
            decision: None,
        };
        m.new_view();
        m
    }
}

// fn matching_msg(msg: &Message, phase: Phase, view: u64) -> bool {
//     msg.phase == phase && msg.view_number == view
// }

fn matching_qc(qc: &Option<QuorumCert>, phase: Phase, view: u64) -> bool {
    match qc {
        None => view == 0 && (phase == Phase::Prepare),
        Some(qc) => qc.phase == phase && qc.view_number == view,
    }
}

#[derive(Clone)]
pub struct Config {
    pub sender_weight: Arc<dyn Fn(tmelcrypt::Ed25519PK) -> f64 + Send + Sync>,
    pub view_leader: Arc<dyn Fn(u64) -> tmelcrypt::Ed25519PK + Send + Sync>,
    pub is_valid_prop: Arc<dyn Fn(&[u8]) -> bool + Send + Sync>,
    pub gen_proposal: Arc<dyn Fn() -> Vec<u8> + Send + Sync>,
    pub my_sk: tmelcrypt::Ed25519SK,
    pub my_pk: tmelcrypt::Ed25519PK,
}

impl Config {
    pub fn qc_tally(&self, qc: &QuorumCert) -> f64 {
        qc.signatures
            .iter()
            .map(|sig| (self.sender_weight)(sig.sender))
            .sum()
    }
}
