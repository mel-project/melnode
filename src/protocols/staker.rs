use blkstructs::Transaction;
use smol::channel::Sender;

use crate::SharedStorage;

/// This encapsulates the staker-specific peer-to-peer.
pub struct StakerProtocol {
    _network_task: smol::Task<()>,
}

struct StakerResponder {
    network: melnet::NetState,
    storage: SharedStorage,
}
