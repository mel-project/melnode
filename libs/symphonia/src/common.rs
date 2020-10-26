use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::{Deserialize, Serialize};

pub const BFT_THRESHOLD: f64 = 0.7;

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub phase: Phase,
    pub node: Node,
    pub justify: Option<QuorumCert>,
    pub sender: tmelcrypt::Ed25519PK,
    pub view_number: u64,
    pub partial_sig: Option<Vec<u8>>,
}

impl Message {
    pub fn sign(self, sk: tmelcrypt::Ed25519SK) -> SignedMessage {
        let msg_bts = bincode::serialize(&self).unwrap();
        let sig = sk.sign(&msg_bts);
        SignedMessage {
            msg: self,
            signature: sig,
        }
    }

    pub fn validate_vote(&self) -> bool {
        match &self.partial_sig {
            Some(sig) => {
                let msg = bincode::serialize(&PVN {
                    phase: self.phase,
                    view_number: self.view_number,
                    node: self.node.clone(),
                })
                .unwrap();
                self.sender.verify(&msg, &sig)
            }
            None => false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SignedMessage {
    pub msg: Message,
    pub signature: Vec<u8>,
}

impl SignedMessage {
    pub fn validate(self) -> Option<Message> {
        let msg_bts = bincode::serialize(&self.msg).unwrap();
        if self.msg.sender.verify(&msg_bts, &self.signature) {
            Some(self.msg)
        } else {
            None
        }
    }
}

#[derive(
    Clone,
    Debug,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    Copy,
    IntoPrimitive,
    TryFromPrimitive,
    Serialize,
    Deserialize,
)]
#[repr(u8)]
pub enum Phase {
    NewView = 0x01,
    Prepare = 0x02,
    PreCommit = 0x03,
    Commit = 0x04,
    Decide = 0x05,
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
pub struct QuorumCert {
    pub phase: Phase,
    pub view_number: u64,
    pub node: Node,
    pub signatures: Vec<QCSig>,
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
pub struct QCSig {
    pub sender: tmelcrypt::Ed25519PK,
    pub signature: Vec<u8>,
}

impl QuorumCert {
    pub fn new(votes: &[Message]) -> Self {
        assert!(!votes.is_empty());
        let mut sigs = Vec::new();
        // assert that all the votes are valid
        for v in votes.iter() {
            assert_eq!(v.phase, votes[0].phase);
            assert_eq!(v.view_number, votes[0].view_number);
            assert_eq!(v.node, votes[0].node);
            assert!(v.validate_vote());
            sigs.push(QCSig {
                signature: v.partial_sig.clone().unwrap(),
                sender: v.sender,
            });
        }
        QuorumCert {
            phase: votes[0].phase,
            view_number: votes[0].view_number,
            node: votes[0].node.clone(),
            signatures: sigs,
        }
    }
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct Node {
    pub parent_hash: tmelcrypt::HashVal,
    pub prop: Vec<u8>,
}

impl Node {
    pub fn create_leaf(parent: Option<&Node>, prop: Vec<u8>) -> Self {
        Node {
            parent_hash: match parent {
                None => tmelcrypt::HashVal([0; 32]),
                Some(p) => p.hash(),
            },
            prop,
        }
    }
    pub fn hash(&self) -> tmelcrypt::HashVal {
        tmelcrypt::hash_single(&bincode::serialize(self).unwrap())
    }
}

#[derive(Serialize, Deserialize)]
pub struct PVN {
    pub phase: Phase,
    pub view_number: u64,
    pub node: Node,
}
