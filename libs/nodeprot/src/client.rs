use std::{
    collections::BTreeMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use autosmt::{CompressedProof, FullProof};
use blkstructs::{
    CoinDataHeight, CoinID, ConsensusProof, Header, NetID, PoolState, SmtMapping, StakeDoc,
    StakeMapping, Transaction, STAKE_EPOCH,
};
use melnet::MelnetError;
use serde::{de::DeserializeOwned, Serialize};
use tmelcrypt::HashVal;

use crate::{AbbreviatedBlock, NodeRequest, StateSummary, Substate};

/// A higher-level client that validates all information.
#[derive(Debug, Clone)]
pub struct ValClient {
    raw: NodeClient,
    trusted_height: Arc<Mutex<Option<(u64, HashVal)>>>,
}

impl ValClient {
    /// Creates a new ValClient.
    pub fn new(netid: NetID, remote: SocketAddr) -> Self {
        let raw = NodeClient::new(netid, remote);
        Self {
            raw,
            trusted_height: Default::default(),
        }
    }

    /// Trust a height.
    pub fn trust(&self, height: u64, header_hash: HashVal) {
        let mut old_trusted = self.trusted_height.lock().unwrap();
        if let Some((old_height, old_hash)) = old_trusted.as_mut() {
            if height > *old_height {
                *old_height = height;
                *old_hash = header_hash
            }
        } else {
            *old_trusted = Some((height, header_hash))
        }
    }

    /// Obtains the latest validated snapshot. Use this method first to get something to validate info against.
    pub async fn snapshot_latest(&self) -> melnet::Result<ValClientSnapshot> {
        self.trust_latest().await;
        self.snapshot().await
    }

    // trust latest height
    async fn trust_latest(&self) {
        let summary = self.raw.get_summary().await.unwrap(); // Add error handling
        self.trust(summary.height, summary.header.hash());
    }

    /// Obtains a validated snapshot based on what height was trusted.
    pub async fn snapshot(&self) -> melnet::Result<ValClientSnapshot> {
        let summary = self.raw.get_summary().await?;
        let (height, stakers) = self.get_trusted_stakers().await?;
        if summary.height / STAKE_EPOCH > height / STAKE_EPOCH + 1 {
            // TODO: Is this the correct condition?
            return Err(MelnetError::Custom(format!(
                "trusted height {} in epoch {} but remote height {} in epoch {}",
                height,
                height / STAKE_EPOCH,
                summary.height,
                summary.height / STAKE_EPOCH
            )));
        }
        // we use the stakers to validate the latest summary
        let mut total_votes = 0.0;
        for doc in stakers.val_iter() {
            if let Some(sig) = summary.proof.get(&doc.pubkey) {
                if doc.pubkey.verify(&summary.header.hash(), sig) {
                    total_votes += stakers.vote_power(summary.height / STAKE_EPOCH, doc.pubkey);
                }
            }
        }
        if total_votes < 0.7 {
            return Err(MelnetError::Custom(format!(
                "remote height {} has insufficient votes",
                summary.height
            )));
        }
        Ok(ValClientSnapshot {
            height: summary.height,
            header: summary.header,
            raw: self.raw.clone(),
        })
    }

    /// Helper function to obtain the trusted staker set.
    async fn get_trusted_stakers(&self) -> melnet::Result<(u64, StakeMapping)> {
        let (trusted_height, trusted_hash) = self.trusted_height.lock().unwrap().unwrap();
        let temp_forest = autosmt::Forest::load(autosmt::MemDB::default());
        let stakers = self.raw.get_stakers_raw(trusted_height).await?;
        // first obtain trusted SMT branch
        let (abbr_block, _) = self.raw.get_abbr_block(trusted_height).await?;
        if abbr_block.header.hash() != trusted_hash {
            return Err(MelnetError::Custom(
                "remote block contradicted trusted block hash".into(),
            ));
        }
        let trusted_stake_hash = abbr_block.header.stake_doc_hash;
        let mut mapping = temp_forest.get_tree(HashVal::default());
        for (k, v) in stakers {
            mapping = mapping.set(k, &v);
        }
        if mapping.root_hash() != trusted_stake_hash {
            return Err(MelnetError::Custom(
                "remote staker set contradicted valid header".into(),
            ));
        }
        Ok((trusted_height, SmtMapping::new(mapping)))
    }
}

/// A "snapshot" of the state at a given state. It essentially encapsulates a NodeClient and a trusted header.
pub struct ValClientSnapshot {
    height: u64,
    header: Header,
    raw: NodeClient,
}

impl ValClientSnapshot {
    /// Gets an older snapshot.
    pub async fn get_older(&self, old_height: u64) -> melnet::Result<Self> {
        if old_height > self.height {
            return Err(MelnetError::Custom("cannot travel into the future".into()));
        }
        // Get an SMT branch
        let val = self
            .get_smt_value(
                Substate::History,
                tmelcrypt::hash_single(&stdcode::serialize(&old_height).unwrap()),
            )
            .await?;
        let old_elem: Header = stdcode::deserialize(&val)
            .map_err(|_| MelnetError::Custom("could not deserialize old header".into()))?;
        // this can never possibly be bad unless everything is horribly untrustworthy
        assert_eq!(old_elem.height, old_height);
        Ok(Self {
            height: old_height,
            header: old_elem,
            raw: self.raw.clone(),
        })
    }

    /// Gets the header.
    pub fn header(&self) -> Header {
        self.header
    }

    /// Gets a historical header.
    pub async fn get_history(&self, height: u64) -> melnet::Result<Header> {
        self.get_smt_value_serde(Substate::History, height).await
    }

    /// Gets a coin.
    pub async fn get_coin(&self, coinid: CoinID) -> melnet::Result<CoinDataHeight> {
        self.get_smt_value_serde(Substate::Coins, coinid).await
    }

    /// Gets a pool info.
    pub async fn get_pool(&self, denom: &[u8]) -> melnet::Result<PoolState> {
        self.get_smt_value_serde(Substate::Pools, denom).await
    }

    /// Gets a stake info.
    pub async fn get_stake(&self, staking_txhash: HashVal) -> melnet::Result<StakeDoc> {
        self.get_smt_value_serde(Substate::Stakes, staking_txhash)
            .await
    }

    /// Gets a transaction.
    pub async fn get_transaction(&self, txhash: HashVal) -> melnet::Result<Transaction> {
        self.get_smt_value_serde(Substate::Transactions, txhash)
            .await
    }

    /// Helper for serde.
    async fn get_smt_value_serde<S: Serialize, D: DeserializeOwned>(
        &self,
        substate: Substate,
        key: S,
    ) -> melnet::Result<D> {
        let val = self
            .get_smt_value(
                substate,
                tmelcrypt::hash_single(&stdcode::serialize(&key).unwrap()),
            )
            .await?;
        let val = stdcode::deserialize(&val)
            .map_err(|_| MelnetError::Custom("fatal deserialization error".into()))?;
        Ok(val)
    }

    /// Gets a local SMT branch, validated.
    pub async fn get_smt_value(&self, substate: Substate, key: HashVal) -> melnet::Result<Vec<u8>> {
        let verify_against = match substate {
            Substate::Coins => self.header.coins_hash,
            Substate::History => self.header.history_hash,
            Substate::Pools => self.header.pools_hash,
            Substate::Stakes => self.header.stake_doc_hash,
            Substate::Transactions => self.header.transactions_hash,
        };
        let (val, branch) = self.raw.get_smt_branch(self.height, substate, key).await?;
        if !branch.verify(verify_against, key, &val) {
            return Err(MelnetError::Custom("unable to verify merkle proof".into()));
        }
        Ok(val)
    }
}

/// A client to a particular node server.
#[derive(Debug, Clone)]
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

    /// Gets the stakers, **as the raw SMT mapping**
    pub async fn get_stakers_raw(&self, height: u64) -> melnet::Result<BTreeMap<HashVal, Vec<u8>>> {
        stdcode::deserialize(&self.request(NodeRequest::GetStakersRaw(height)).await?)
            .map_err(|e| melnet::MelnetError::Custom(e.to_string()))
    }
}
