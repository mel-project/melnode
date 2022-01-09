use std::{
    collections::BTreeMap,
    net::SocketAddr,
    time::{Duration, Instant},
};

use anyhow::Context;
use futures_util::{StreamExt, TryStreamExt};
use novasmt::CompressedProof;
use themelio_stf::{
    AbbrBlock, Block, BlockHeight, ConsensusProof, NetID, SealedState, Transaction,
};

use crate::storage::{MeshaCas, NodeStorage};
use melnet::MelnetError;
use smol::net::TcpListener;
use smol_timeout::TimeoutExt;
use themelio_nodeprot::{NodeClient, NodeResponder, NodeServer, StateSummary, Substate};
use tmelcrypt::HashVal;

/// This encapsulates the node peer-to-peer for both auditors and stakers..
pub struct NodeProtocol {
    _network_task: smol::Task<()>,
    _blksync_task: smol::Task<()>,
}

fn netname(network: NetID) -> &'static str {
    match network {
        NetID::Mainnet => "mainnet-node",
        NetID::Testnet => "testnet-node",
        _ => Box::leak(Box::new(format!("{:?}", network))),
    }
}

impl NodeProtocol {
    /// Creates a new AuditorProtocol listening on the given address with the given AuditorState.
    pub fn new(
        netid: NetID,
        listen_addr: SocketAddr,
        advertise_addr: Option<SocketAddr>,
        bootstrap: Vec<SocketAddr>,
        storage: NodeStorage,
    ) -> Self {
        let network = melnet::NetState::new_with_name(netname(netid));
        for addr in bootstrap {
            network.add_route(addr);
        }
        if let Some(advertise_addr) = advertise_addr {
            network.add_route(advertise_addr);
        }
        let responder = AuditorResponder::new(netid, storage.clone());
        network.listen("node", NodeResponder::new(responder));
        let _network_task = smolscale::spawn({
            let network = network.clone();
            async move {
                let listener = TcpListener::bind(listen_addr).await.unwrap();
                network.run_server(listener).await;
            }
        });
        let _blksync_task = smolscale::spawn(blksync_loop(netid, network, storage));
        Self {
            _network_task,
            _blksync_task,
        }
    }
}

#[tracing::instrument(skip(network, storage))]
async fn blksync_loop(netid: NetID, network: melnet::NetState, storage: NodeStorage) {
    let tag = || format!("blksync@{:?}", storage.highest_state().header().height);
    const SLOW_TIME: Duration = Duration::from_millis(500);
    const FAST_TIME: Duration = Duration::from_millis(500);
    loop {
        let routes = network.routes();
        let random_peer = routes.first().cloned();
        if let Some(peer) = random_peer {
            // #[cfg(not(feature = "metrics"))]
            // log::debug!(
            //     "{}: picked random peer {} out of {} peers {:?} for blksync",
            //     tag(),
            //     peer,
            //     routes.len(),
            //     routes
            // );
            // #[cfg(feature = "metrics")]
            // log::debug!(
            //     "hostname={} public_ip={} {}: picked random peer {} out of {} peers {:?} for blksync",
            //     crate::prometheus::HOSTNAME.as_str(), crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
            //     tag(),
            //     peer,
            //     routes.len(),
            //     routes
            // );

            let client = NodeClient::new(netid, peer);

            let res = attempt_blksync(peer, &client, &storage).await;
            match res {
                Err(e) => {
                    #[cfg(not(feature = "metrics"))]
                    log::warn!("{}: failed to blksync with {}: {:?}", tag(), peer, e);
                    #[cfg(feature = "metrics")]
                    log::warn!(
                        "hostname={} public_ip={} {}: failed to blksync with {}: {:?}",
                        crate::prometheus::HOSTNAME.as_str(),
                        crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
                        tag(),
                        peer,
                        e
                    );
                    smol::Timer::after(FAST_TIME).await;
                }
                Ok(blklen) => {
                    if blklen > 0 {
                        #[cfg(not(feature = "metrics"))]
                        log::debug!("synced to height {}", storage.highest_height());
                        #[cfg(feature = "metrics")]
                        log::warn!(
                            "hostname={} public_ip={} synced to height {}",
                            crate::prometheus::HOSTNAME.as_str(),
                            crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
                            storage.highest_height()
                        );
                        smol::Timer::after(FAST_TIME).await;
                    } else {
                        smol::Timer::after(SLOW_TIME).await;
                    }
                }
            }
        } else {
            smol::Timer::after(SLOW_TIME).await;
        }
    }
}

/// Attempts a sync using the given given node client.
async fn attempt_blksync(
    addr: SocketAddr,
    client: &NodeClient,
    storage: &NodeStorage,
) -> anyhow::Result<usize> {
    let their_highest = client
        .get_summary()
        .await
        .context("cannot get their highest block")?
        .height;
    let my_highest = storage.highest_height();
    if their_highest <= my_highest {
        return Ok(0);
    }
    let height_stream =
        futures_util::stream::iter((my_highest.0..=their_highest.0).skip(1)).map(BlockHeight);
    let lookup_tx = |tx| storage.mempool().lookup_recent_tx(tx);
    let mut result_stream = height_stream
        .map(Ok::<_, anyhow::Error>)
        .try_filter_map(|height| async move {
            Ok(Some(async move {
                let result = client
                    .get_full_block(height, &lookup_tx)
                    .timeout(Duration::from_secs(15))
                    .await
                    .context("timeout")??;
                if result.0.header.height != height {
                    anyhow::bail!("WANTED BLK {}, got {}", height, result.0.header.height);
                }
                Ok(result)
            }))
        })
        .try_buffered(32)
        .boxed();
    let mut toret = 0;
    while let Some(res) = result_stream.try_next().await? {
        let (block, proof): (Block, ConsensusProof) = res;
        #[cfg(not(feature = "metrics"))]
        log::debug!(
            "fully resolved block {} from peer {}",
            block.header.height,
            addr
        );
        #[cfg(feature = "metrics")]
        log::debug!(
            "hostname={} public_ip={} fully resolved block {} from peer {}",
            crate::prometheus::HOSTNAME.as_str(),
            crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
            block.header.height,
            addr
        );

        storage
            .apply_block(block, proof)
            .context("could not apply a resolved block")?;
        toret += 1;
    }
    Ok(toret)
}

struct AuditorResponder {
    network: NetID,
    storage: NodeStorage,
}

impl NodeServer<MeshaCas> for AuditorResponder {
    fn send_tx(&self, state: melnet::NetState, tx: Transaction) -> melnet::Result<()> {
        let start = Instant::now();
        let post_lock = Instant::now();
        self.storage
            .mempool_mut()
            .apply_transaction(&tx)
            .map_err(|e| {
                // log::warn!("cannot apply tx: {:?}", e);
                MelnetError::Custom(e.to_string())
            })?;
        #[cfg(not(feature = "metrics"))]
        log::debug!(
            "txhash {}.. inserted ({:?}, {:?} locking, {:?} applying)",
            &tx.hash_nosigs().to_string()[..10],
            start.elapsed(),
            post_lock - start,
            post_lock.elapsed()
        );
        #[cfg(feature = "metrics")]
        log::debug!(
            "hostname={} public_ip={} txhash {}.. inserted ({:?}, {:?} locking, {:?} applying)",
            crate::prometheus::HOSTNAME.as_str(),
            crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
            &tx.hash_nosigs().to_string()[..10],
            start.elapsed(),
            post_lock - start,
            post_lock.elapsed()
        );

        // log::debug!("about to broadcast txhash {:?}", tx.hash_nosigs());
        for neigh in state.routes().iter().take(4).cloned() {
            let tx = tx.clone();
            let network = self.network;
            // log::debug!("bcast {:?} => {:?}", tx.hash_nosigs(), neigh);
            smolscale::spawn(async move {
                NodeClient::new(network, neigh)
                    .send_tx(tx)
                    .timeout(Duration::from_secs(10))
                    .await
            })
            .detach();
        }
        Ok(())
    }

    fn get_abbr_block(&self, height: BlockHeight) -> melnet::Result<(AbbrBlock, ConsensusProof)> {
        let state = self
            .storage
            .get_state(height)
            .ok_or_else(|| MelnetError::Custom(format!("block {} not confirmed yet", height)))?;
        let proof = self
            .storage
            .get_consensus(height)
            .ok_or_else(|| MelnetError::Custom(format!("block {} not confirmed yet", height)))?;
        Ok((state.to_block().abbreviate(), proof))
    }

    fn get_summary(&self) -> melnet::Result<StateSummary> {
        let highest = self.storage.highest_state();
        let proof = self
            .storage
            .get_consensus(highest.header().height)
            .unwrap_or_default();
        Ok(StateSummary {
            netid: self.network,
            height: self.storage.highest_height(),
            header: highest.header(),
            proof,
        })
    }

    fn get_state(&self, height: BlockHeight) -> melnet::Result<SealedState<MeshaCas>> {
        self.storage
            .get_state(height)
            .ok_or_else(|| melnet::MelnetError::Custom("no such height".into()))
    }

    fn get_smt_branch(
        &self,
        height: BlockHeight,
        elem: Substate,
        key: HashVal,
    ) -> melnet::Result<(Vec<u8>, CompressedProof)> {
        let state = self
            .storage
            .get_state(height)
            .ok_or_else(|| MelnetError::Custom(format!("block {} not confirmed yet", height)))?;
        let tree = match elem {
            Substate::Coins => &state.inner_ref().coins.mapping,
            Substate::History => &state.inner_ref().history.mapping,
            Substate::Pools => &state.inner_ref().pools.mapping,
            Substate::Stakes => &state.inner_ref().stakes.mapping,
            Substate::Transactions => &state.inner_ref().transactions.mapping,
        };
        let (v, proof) = tree.get_with_proof(key.0);
        if !proof.verify(tree.root_hash(), key.0, &v) {
            panic!(
                "get_smt_branch({}, {:?}, {:?}) => {} failed",
                height,
                elem,
                key,
                hex::encode(&v)
            )
        }
        Ok((v.to_vec(), proof.compress()))
    }

    fn get_stakers_raw(&self, height: BlockHeight) -> melnet::Result<BTreeMap<HashVal, Vec<u8>>> {
        let state = self
            .storage
            .get_state(height)
            .ok_or_else(|| MelnetError::Custom(format!("block {} not confirmed yet", height)))?;
        let mut accum = BTreeMap::new();
        for (k, v) in state.inner_ref().stakes.mapping.iter() {
            accum.insert(HashVal(k), v.to_vec());
        }
        Ok(accum)
    }
}

impl AuditorResponder {
    fn new(network: NetID, storage: NodeStorage) -> Self {
        Self { network, storage }
    }
}
