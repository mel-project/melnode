use std::collections::BTreeMap;

use autosmt::CompressedProof;
use blkstructs::{ConsensusProof, StakeDoc, Transaction};
use melnet::Request;
use tmelcrypt::HashVal;

use crate::{AbbreviatedBlock, NodeRequest, StateSummary, Substate};

/// This trait represents a server of Themelio's node protocol. Actual nodes should implement this.
pub trait NodeServer {
    /// Broadcasts a transaction to the network
    fn send_tx(&self, state: melnet::NetState, tx: Transaction) -> melnet::Result<()>;

    /// Gets an "abbreviated block"
    fn get_abbr_block(&self, height: u64) -> melnet::Result<(AbbreviatedBlock, ConsensusProof)>;

    /// Gets a state summary
    fn get_summary(&self) -> melnet::Result<StateSummary>;

    /// Gets an SMT branch
    fn get_smt_branch(
        &self,
        height: u64,
        elem: Substate,
        key: HashVal,
    ) -> melnet::Result<(Vec<u8>, CompressedProof)>;

    /// Gets stakers
    fn get_stakers_raw(&self, height: u64) -> melnet::Result<BTreeMap<HashVal, Vec<u8>>>;
}

/// This is a melnet responder that wraps a NodeServer.
pub struct NodeResponder<S: NodeServer> {
    server: S,
}

impl<S: NodeServer> NodeResponder<S> {
    /// Creates a new NodeResponder from something that implements NodeServer.
    pub fn new(server: S) -> Self {
        Self { server }
    }
}

impl<S: NodeServer> melnet::Responder<NodeRequest, Vec<u8>> for NodeResponder<S> {
    fn respond(&mut self, req: Request<NodeRequest, Vec<u8>>) {
        let state = req.state.clone();
        match req.body.clone() {
            NodeRequest::SendTx(tx) => req.respond(self.server.send_tx(state, tx).map(|_| vec![])),
            NodeRequest::GetSummary => req.respond(
                self.server
                    .get_summary()
                    .map(|sum| stdcode::serialize(&sum).unwrap()),
            ),
            NodeRequest::GetAbbrBlock(height) => req.respond(
                self.server
                    .get_abbr_block(height)
                    .map(|blk| stdcode::serialize(&blk).unwrap()),
            ),
            NodeRequest::GetSmtBranch(height, elem, key) => req.respond(
                self.server
                    .get_smt_branch(height, elem, key)
                    .map(|v| stdcode::serialize(&v).unwrap()),
            ),
            NodeRequest::GetStakersRaw(height) => req.respond(
                self.server
                    .get_stakers_raw(height)
                    .map(|v| stdcode::serialize(&v).unwrap()),
            ),
        }
    }
}
