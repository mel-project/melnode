use std::collections::HashMap;

use crate::msg::SignedMessage;
use serde::{Deserialize, Serialize};
use tmelcrypt::Ed25519PK;

#[derive(Default, Debug)]
pub struct MsgState {
    inner: HashMap<Ed25519PK, Vec<SignedMessage>>,
}

impl MsgState {
    /// Inserts one thing into the MsgState. Returns whether or not this was successful.
    pub fn insert(&mut self, msg: SignedMessage) -> bool {
        if msg.body().is_none() {
            return false;
        }
        let vec = self.inner.entry(msg.sender).or_default();
        if let Some(last) = vec.last() {
            if last.sequence >= msg.sequence {
                return false;
            }
        }
        vec.push(msg);
        true
    }

    /// Takes a serializable "snapshot" of the MsgState.
    pub fn snapshot(&self) -> MsgStateStatus {
        let mut toret = HashMap::new();
        for (k, v) in self.inner.iter() {
            if let Some(last) = v.last() {
                toret.insert(*k, last.sequence);
            }
        }
        MsgStateStatus { last_seqs: toret }
    }

    /// Returns the diff between this MsgState and the given status.
    pub fn oneside_diff(&self, their_status: MsgStateStatus) -> MsgStateDiff {
        // TODO better indexing
        let mut messages = HashMap::new();
        for (pk, msgs) in self.inner.iter() {
            let their_min = their_status.last_seqs.get(pk).cloned().unwrap_or_default();
            messages.insert(
                *pk,
                msgs.iter()
                    .filter(|v| v.sequence > their_min)
                    .cloned()
                    .collect(),
            );
        }
        MsgStateDiff { messages }
    }

    /// Applies a diff to this MsgState.
    pub fn apply_diff(&mut self, their_diff: MsgStateDiff) -> Vec<SignedMessage> {
        let mut toret = vec![];
        for (pk, msgs) in their_diff.messages {
            let vec = self.inner.entry(pk).or_default();
            for msg in msgs {
                let our_last = vec.last().map(|v| v.sequence).unwrap_or_default();
                if msg.sequence > our_last {
                    log::warn!("apply_diff applying PK={:?}, SEQ={}", pk, msg.sequence);
                    vec.push(msg.clone());
                    toret.push(msg);
                }
            }
        }
        toret
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MsgStateStatus {
    last_seqs: HashMap<Ed25519PK, u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MsgStateDiff {
    messages: HashMap<Ed25519PK, Vec<SignedMessage>>,
}
