use num_enum::{IntoPrimitive, TryFromPrimitive};
use rlp::{Decodable, Encodable};
use rlp_derive::*;
use std::convert::TryFrom;

pub const BFT_THRESHOLD: f64 = 0.7;

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, RlpEncodable, RlpDecodable)]
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
        let msg_bts = rlp::encode(&self);
        let sig = sk.sign(&msg_bts);
        SignedMessage {
            msg: self,
            signature: sig,
        }
    }

    pub fn validate_vote(&self) -> bool {
        match &self.partial_sig {
            Some(sig) => {
                let msg = rlp::encode(&PVN {
                    phase: self.phase,
                    view_number: self.view_number,
                    node: self.node.clone(),
                });
                self.sender.verify(&msg, &sig)
            }
            None => false,
        }
    }
}

#[derive(RlpEncodable, RlpDecodable, Clone, Debug)]
pub struct SignedMessage {
    pub msg: Message,
    pub signature: Vec<u8>,
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

type Vecu8 = Vec<u8>;
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, RlpEncodable, RlpDecodable)]
pub struct QuorumCert {
    pub phase: Phase,
    pub view_number: u64,
    pub node: Node,
    pub signatures: Vec<Vecu8>,
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
            sigs.push(v.partial_sig.clone().unwrap());
        }
        QuorumCert {
            phase: votes[0].phase,
            view_number: votes[0].view_number,
            node: votes[0].node.clone(),
            signatures: sigs,
        }
    }
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, RlpEncodable, RlpDecodable, Default)]
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
        tmelcrypt::hash_single(&rlp::encode(self))
    }
}

#[derive(RlpEncodable)]
pub struct PVN {
    pub phase: Phase,
    pub view_number: u64,
    pub node: Node,
}
