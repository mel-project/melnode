use blkstructs::{AbbrBlock, Block};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::{sync::atomic::AtomicU64, time::SystemTime};
use tmelcrypt::{Ed25519PK, Ed25519SK, HashVal};

/// A message signer
pub struct Signer {
    sk: Ed25519SK,
    sequence: AtomicU64,
}

impl Signer {
    /// Create a new signer
    pub fn new(sk: Ed25519SK) -> Self {
        let starting_sequence = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            sk,
            sequence: AtomicU64::new(starting_sequence),
        }
    }

    /// Signs a message
    pub fn sign(&self, msg: Message) -> SignedMessage {
        let sequence = self
            .sequence
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let to_sign =
            tmelcrypt::hash_keyed(b"ns-msg", &stdcode::serialize(&(sequence, &msg)).unwrap());
        let signature = self.sk.sign(&to_sign).into();
        SignedMessage {
            sender: self.sk.to_public(),
            signature,
            sequence,
            body: msg,
        }
    }
}

/// Message sent *to* a node
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SignedMessage {
    pub sender: Ed25519PK,
    pub signature: Bytes, // over (sequence, body)
    pub sequence: u64,    // monotonically increasing. clock+ctr
    body: Message,
}

impl SignedMessage {
    /// Gets the body out, verifying the signature along the way.
    pub fn body(&self) -> Option<&Message> {
        let to_sign = tmelcrypt::hash_keyed(
            b"ns-msg",
            &stdcode::serialize(&(self.sequence, &self.body)).unwrap(),
        );
        let correct = self.sender.verify(&to_sign, &self.signature);
        if !correct {
            return None;
        }
        Some(&self.body)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Message {
    Proposal(ProposalMsg),
    Vote(VoteMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProposalMsg {
    pub proposal: AbbrBlock,
    pub last_nonempty: Option<(u64, HashVal)>,
}

/// An actualized proposal
#[derive(Clone, Debug)]
pub struct ActualProposal {
    pub block: Block,
    pub last_nonempty: Option<(u64, HashVal)>,
}

impl ActualProposal {
    /// What does it extend from?
    pub fn extends_from(&self) -> HashVal {
        if let Some((_, val)) = self.last_nonempty {
            val
        } else {
            self.block.header.previous
        }
    }

    /// Height
    pub fn height(&self) -> u64 {
        self.block.header.height
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VoteMsg {
    pub voting_for: HashVal,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GetConfirmMsg {
    pub height: u64,
    pub hash: HashVal,
}
