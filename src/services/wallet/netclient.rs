use crate::protocols::{AbbreviatedBlock, NODE_NETNAME};
use blkstructs::{CoinDataHeight, CoinID, Header, Transaction};
use std::time::Instant;
use std::{net::SocketAddr, time::Duration};
use symphonia::QuorumCert;
use tmelcrypt::HashVal;

/// A network client with some in-memory caching. Abstracts away melnet RPC calls.
pub struct NetClient {
    remote: SocketAddr,
    // cached variables
    last_header: Option<Header>,
    cache_date: Option<Instant>,
}

impl NetClient {
    /// Create a new network client.
    pub fn new(remote: SocketAddr) -> Self {
        Self {
            remote,
            last_header: None,
            cache_date: None,
        }
    }
    // update last header and cache state variables
    async fn sync_with_net(&mut self) -> anyhow::Result<()> {
        let remote_state: (AbbreviatedBlock, QuorumCert) = melnet::g_client()
            .request(self.remote, NODE_NETNAME, "get_last_state", ())
            .await?;
        log::warn!("not actually validating QuorumCert for last state");
        self.last_header = Some(remote_state.0.header);
        self.cache_date = Some(Instant::now());
        Ok(())
    }
    // calls sync with net and verify header (dont need to do)
    /// Obtain and verify the latest header.
    pub async fn last_header(&mut self) -> anyhow::Result<(Header, Instant)> {
        loop {
            if let Some(header) = &self.last_header {
                let cache_date = self.cache_date.unwrap();
                if Instant::now() < cache_date + Duration::from_secs(15) {
                    return Ok((*header, cache_date));
                }
            }
            self.sync_with_net().await?;
        }
    }
    // translate the master client to current system (be in protocols folder)
    /// Get and verify a specific coin.
    pub async fn get_coin(
        &mut self,
        header: Header,
        coin: CoinID,
    ) -> anyhow::Result<(Option<CoinDataHeight>, autosmt::FullProof)> {
        // log::debug!("get_coin at height {} for coin {:?}", header.height, coin);
        let res: (Option<CoinDataHeight>, autosmt::CompressedProof) = melnet::g_client()
            .request(
                self.remote,
                NODE_NETNAME,
                "get_coin_at",
                (header.height, coin),
            )
            .await?;
        let proof = res
            .1
            .decompress()
            .ok_or_else(|| anyhow::anyhow!("invalid compressed proof"))?;
        // proof.verify(
        //     _header.coins_hash,
        //     tmelcrypt::hash_single(&bincode::serialize(value)),
        //     val,
        // )
        // log::warn!("not verifying merkle tree branch");
        Ok((res.0, proof))
    }

    /// Get and verify a specific transaction at a specific height
    pub async fn get_tx(
        &mut self,
        header: Header,
        txhash: HashVal,
    ) -> anyhow::Result<(Option<Transaction>, autosmt::FullProof)> {
        let res: (Option<Transaction>, autosmt::CompressedProof) = melnet::g_client()
            .request(
                self.remote,
                NODE_NETNAME,
                "get_tx_at",
                (header.height, txhash),
            )
            .await?;
        let proof = res
            .1
            .decompress()
            .ok_or_else(|| anyhow::anyhow!("invalid compressed proof"))?;
        // proof.verify(
        //     _header.coins_hash,
        //     tmelcrypt::hash_single(&bincode::serialize(value)),
        //     val,
        // )
        // log::warn!("not verifying merkle tree branch");
        Ok((res.0, proof))
    }

    /// Actually broadcast a transaction!
    pub async fn broadcast_tx(&mut self, tx: Transaction) -> anyhow::Result<()> {
        Ok(melnet::g_client()
            .request(self.remote, NODE_NETNAME, "send_tx", tx)
            .await?)
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     // TODO: Fix these require endpoint to be running so can't be run in CI?
//     // Perhaps these need to be run as integration tests after a deploy
//     #[test]
//     fn last_header() {
//         smol::block_on(async {
//             let mut client = NetClient::new("94.237.109.116:11814".parse().unwrap());
//             dbg!(client.last_header().await.unwrap());
//         });
//     }
//
//     #[test]
//     fn get_coin() {
//         smol::block_on(async {
//             let mut client = NetClient::new("94.237.109.116:11814".parse().unwrap());
//             let header = client.last_header().await.unwrap().0;
//             dbg!(
//                 client
//                     .get_coin(header, CoinID::zero_zero())
//                     .await
//                     .unwrap()
//                     .0
//             );
//         });
//     }
// }
