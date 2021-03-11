use std::net::SocketAddr;

use autosmt::{CompressedProof, FullProof};
use blkstructs::{ConsensusProof, NetID, Transaction};
use tmelcrypt::HashVal;

use crate::{AbbreviatedBlock, NodeRequest, StateSummary, Substate};

/// A client to a particular node server.
pub struct NodeClient {
    remote: SocketAddr,
    netname: String,
}

impl NodeClient {
    /// Creates as new NodeClient
    pub fn new(netid: NetID, remote: SocketAddr) -> Self {
        let netname = match netid {
            NetID::Mainnet => "mainnet-node",
            NetID::Testnet => "testnet-node",
        }
        .to_string();
        Self { remote, netname }
    }

    /// Helper function to do a request.
    async fn request(&self, req: NodeRequest) -> melnet::Result<Vec<u8>> {
        melnet::g_client()
            .request(self.remote, &self.netname, "node", req)
            .await
    }

    /// Sends a tx.
    pub async fn send_tx(&self, tx: Transaction) -> melnet::Result<()> {
        self.request(NodeRequest::SendTx(tx)).await?;
        Ok(())
    }

    /// Gets a summary of the state.
    pub async fn get_summary(&self) -> melnet::Result<StateSummary> {
        stdcode::deserialize(&self.request(NodeRequest::GetSummary).await?)
            .map_err(|e| melnet::MelnetError::Custom(e.to_string()))
    }

    /// Gets an "abbreviated block".
    pub async fn get_abbr_block(
        &self,
        height: u64,
    ) -> melnet::Result<(AbbreviatedBlock, ConsensusProof)> {
        stdcode::deserialize(&self.request(NodeRequest::GetAbbrBlock(height)).await?)
            .map_err(|e| melnet::MelnetError::Custom(e.to_string()))
    }

    /// Gets an SMT branch.
    pub async fn get_smt_branch(
        &self,
        height: u64,
        elem: Substate,
        key: HashVal,
    ) -> melnet::Result<(Vec<u8>, FullProof)> {
        let tuple: (Vec<u8>, CompressedProof) = stdcode::deserialize(
            &self
                .request(NodeRequest::GetSmtBranch(height, elem, key))
                .await?,
        )
        .map_err(|e| melnet::MelnetError::Custom(e.to_string()))?;
        let decompressed = tuple
            .1
            .decompress()
            .ok_or_else(|| melnet::MelnetError::Custom("could not decompress proof".into()))?;
        Ok((tuple.0, decompressed))
    }
}
