use crate::{
    blkidx::BlockIndexer,
    storage::{MeshaCas, NodeStorage},
};

#[cfg(feature = "metrics")]
use crate::prometheus::{AWS_INSTANCE_ID, AWS_REGION};

use std::{
    collections::BTreeMap,
    net::SocketAddr,
    time::{Duration, Instant},
};

use anyhow::Context;
use futures_util::{StreamExt, TryStreamExt};
use novasmt::CompressedProof;
use themelio_stf::SealedState;
use themelio_structs::{AbbrBlock, Block, BlockHeight, ConsensusProof, NetID, Transaction};

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
        index: bool,
    ) -> Self {
        let network = melnet::NetState::new_with_name(netname(netid));
        for addr in bootstrap {
            network.add_route(addr);
        }
        if let Some(advertise_addr) = advertise_addr {
            network.add_route(advertise_addr);
        }
        let responder = AuditorResponder::new(netid, storage.clone(), index);
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
    const FAST_TIME: Duration = Duration::from_millis(500);
    loop {
        let slow_time: Duration = Duration::from_secs_f64(fastrand::f64() * 5.0);
        let routes = network.routes();
        let random_peer = routes.first().cloned();
        if let Some(peer) = random_peer {
            log::debug!("picking peer {} out of peers {:?}", peer, routes);
            let client = NodeClient::new(netid, peer);

            let res = attempt_blksync(peer, &client, &storage).await;
            match res {
                Err(e) => {
                    #[cfg(not(feature = "metrics"))]
                    log::warn!("{}: failed to blksync with {}: {:?}", tag(), peer, e);
                    #[cfg(feature = "metrics")]
                    log::warn!(
                        "hostname={} public_ip={} network={} region={} instance_id={} {}: failed to blksync with {}: {:?}",
                        crate::prometheus::HOSTNAME.as_str(),
                        crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
                        crate::prometheus::NETWORK
                            .read()
                            .expect("Could not get a read lock on NETWORK."),
                        AWS_REGION.read().expect("Could not get a read lock on AWS_REGION"),
                        AWS_INSTANCE_ID.read().expect("Could not get a read lock on AWS_INSTANCE_ID"),
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
                            "hostname={} public_ip={} network={} region={} instance_id={} synced to height {}",
                            crate::prometheus::HOSTNAME.as_str(),
                            crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
                            crate::prometheus::NETWORK
                                .read()
                                .expect("Could not get a read lock on NETWORK."),
                            AWS_REGION.read().expect("Could not get a read lock on AWS_REGION"),
                            AWS_INSTANCE_ID.read().expect("Could not get a read lock on AWS_INSTANCE_ID"),
                            storage.highest_height()
                        );

                        smol::Timer::after(FAST_TIME).await;
                    } else {
                        smol::Timer::after(slow_time).await;
                    }
                }
            }
        } else {
            smol::Timer::after(slow_time).await;
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
    let height_stream = futures_util::stream::iter((my_highest.0..=their_highest.0).skip(1))
        .map(BlockHeight)
        .take(1024);
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
            "hostname={} public_ip={} network={} region={} instance_id={} fully resolved block {} from peer {}",
            crate::prometheus::HOSTNAME.as_str(),
            crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
            crate::prometheus::NETWORK
                .read()
                .expect("Could not get a read lock on NETWORK."),
            AWS_REGION.read().expect("Could not get a read lock on AWS_REGION"),
            AWS_INSTANCE_ID.read().expect("Could not get a read lock on AWS_INSTANCE_ID"),
            block.header.height,
            addr
        );

        storage
            .apply_block(block, proof)
            .await
            .context("could not apply a resolved block")?;
        toret += 1;
    }
    Ok(toret)
}

struct AuditorResponder {
    network: NetID,
    storage: NodeStorage,
    indexer: Option<BlockIndexer>,
}

impl NodeServer<MeshaCas> for AuditorResponder {
    fn send_tx(&self, state: melnet::NetState, tx: Transaction) -> anyhow::Result<()> {
        log::trace!("handling send_tx");
        let start = Instant::now();
        self.storage
            .mempool_mut()
            .apply_transaction(&tx)
            .map_err(|e| {
                log::warn!("cannot apply tx: {:?}", e);
                MelnetError::Custom(e.to_string())
            })?;
        #[cfg(not(feature = "metrics"))]
        log::debug!(
            "txhash {}.. inserted ({:?} applying)",
            &tx.hash_nosigs().to_string()[..10],
            start.elapsed(),
        );
        #[cfg(feature = "metrics")]
        log::debug!(
            "hostname={} public_ip={} network={} region={} instance_id={} txhash {}.. inserted ({:?} applying)",
            crate::prometheus::HOSTNAME.as_str(),
            crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
            crate::prometheus::NETWORK.read().expect("Could not get a read lock on NETWORK."),
            AWS_REGION.read().expect("Could not get a read lock on AWS_REGION"),
            AWS_INSTANCE_ID.read().expect("Could not get a read lock on AWS_INSTANCE_ID"),
            &tx.hash_nosigs().to_string()[..10],
            start.elapsed(),
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

    fn get_abbr_block(&self, height: BlockHeight) -> anyhow::Result<(AbbrBlock, ConsensusProof)> {
        log::trace!("handling get_abbr_block({})", height);
        let state = self
            .storage
            .get_state(height)
            .context(format!("block {} not confirmed yet", height))?;
        let proof = self
            .storage
            .get_consensus(height)
            .context(format!("block {} not confirmed yet", height))?;
        Ok((state.to_block().abbreviate(), proof))
    }

    fn get_summary(&self) -> anyhow::Result<StateSummary> {
        log::trace!("handling get_summary()");
        let highest = self.storage.highest_state();
        let proof = self
            .storage
            .get_consensus(highest.header().height)
            .unwrap_or_default();
        Ok(StateSummary {
            netid: self.network,
            height: highest.inner_ref().height,
            header: highest.header(),
            proof,
        })
    }

    fn get_state(&self, height: BlockHeight) -> anyhow::Result<SealedState<MeshaCas>> {
        log::trace!("handling get_state({})", height);
        self.storage.get_state(height).context("no such height")
    }

    fn get_smt_branch(
        &self,
        height: BlockHeight,
        elem: Substate,
        key: HashVal,
    ) -> anyhow::Result<(Vec<u8>, CompressedProof)> {
        log::trace!("handling get_smt_branch({}, {:?})", height, elem);
        let state = self
            .storage
            .get_state(height)
            .ok_or_else(|| MelnetError::Custom(format!("block {} not confirmed yet", height)))?;
        let tree = match elem {
            Substate::Coins => state.inner_ref().coins.inner(),
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

    fn get_stakers_raw(&self, height: BlockHeight) -> anyhow::Result<BTreeMap<HashVal, Vec<u8>>> {
        let state = self.storage.get_state(height).context("no such height")?;
        let mut accum = BTreeMap::new();
        for (k, v) in state.inner_ref().stakes.mapping.iter() {
            accum.insert(HashVal(k), v.to_vec());
        }
        Ok(accum)
    }

    fn get_some_coins(
        &self,
        height: BlockHeight,
        covhash: themelio_structs::Address,
    ) -> anyhow::Result<Option<Vec<themelio_structs::CoinID>>> {
        Ok(self
            .indexer
            .as_ref()
            .and_then(|s| s.get(height).map(|idx| idx.lookup(covhash))))
    }
}

impl AuditorResponder {
    fn new(network: NetID, storage: NodeStorage, index: bool) -> Self {
        Self {
            network,
            storage: storage.clone(),
            indexer: if index {
                Some(BlockIndexer::new(storage))
            } else {
                None
            },
        }
    }
}
