use crate::storage::{MeshaCas, NodeStorage};

use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime},
};

use novasymph::BlockBuilder;
use once_cell::sync::Lazy;
use smol::prelude::*;
use themelio_stf::SealedState;
use themelio_structs::{Address, Block, BlockHeight, NetID, ProposerAction, Transaction, TxHash};
use tmelcrypt::Ed25519SK;

static MAINNET_START_TIME: Lazy<SystemTime> =
    Lazy::new(|| std::time::UNIX_EPOCH + Duration::from_secs(1618365600)); // Apr 14 2021

static TESTNET_START_TIME: Lazy<SystemTime> =
    Lazy::new(|| std::time::UNIX_EPOCH + Duration::from_secs(1618365600)); // Apr 14 2021

/// This encapsulates the staker-specific peer-to-peer.
pub struct StakerProtocol {
    _network_task: smol::Task<()>,
}

impl StakerProtocol {
    /// Creates a new instance of the staker protocol.
    pub fn new(
        addr: SocketAddr,
        bootstrap: Vec<SocketAddr>,
        storage: NodeStorage,
        my_sk: Ed25519SK,
        payout_address: Address,
        target_fee_multiplier: u128,
    ) -> anyhow::Result<Self> {
        let _network_task = smolscale::spawn(async move {
            loop {
                let x = storage.highest_height();
                smol::Timer::after(Duration::from_secs(10)).await;
                let y = storage.highest_height();
                log::info!(
                    "delta-height = {}; must be less than 5 to start staker",
                    y - x
                );

                if y - x < 5.into() {
                    break;
                }
            }
            loop {
                let genesis_epoch = storage.highest_height().epoch();
                for current_epoch in genesis_epoch.. {
                    log::info!("epoch transitioning into {}!", current_epoch);

                    smol::Timer::after(Duration::from_secs(1)).await;
                    // we race the staker loop with epoch termination. epoch termination for now is just a sleep loop that waits until the last block in the epoch is confirmed.
                    let staker_fut = one_epoch_loop(
                        addr,
                        bootstrap.clone(),
                        storage.clone(),
                        my_sk,
                        payout_address,
                        target_fee_multiplier,
                    );
                    let epoch_termination = async {
                        loop {
                            smol::Timer::after(Duration::from_secs(1)).await;
                            if (storage.highest_height() + 1.into()).epoch() != current_epoch {
                                break Ok(());
                            }
                        }
                    };
                    if let Err(err) = staker_fut.race(epoch_termination).await {
                        log::warn!("staker rebooting: {:?}", err);

                        break;
                    }
                }
            }
        });
        Ok(Self { _network_task })
    }
}

async fn one_epoch_loop(
    addr: SocketAddr,
    bootstrap: Vec<SocketAddr>,
    storage: NodeStorage,
    my_sk: Ed25519SK,
    payout_covhash: Address,
    target_fee_multiplier: u128,
) -> anyhow::Result<()> {
    let genesis = storage.highest_state();
    let start_time = match genesis.inner_ref().network {
        NetID::Mainnet => *MAINNET_START_TIME,
        NetID::Testnet => *TESTNET_START_TIME,
        _ => SystemTime::now() - Duration::from_secs(storage.highest_height().0 * 30),
    };
    let config = novasymph::EpochConfig {
        listen: addr,
        bootstrap,
        genesis,
        start_time,
        interval: Duration::from_secs(30),
        signing_sk: my_sk,
        builder: StorageBlockBuilder {
            storage: storage.clone(),
            payout_covhash,
            target_fee_multiplier,
        }
        .into(),
        get_confirmed: {
            let storage = storage.clone();
            Box::new(move |height: BlockHeight| {
                storage
                    .get_state(height)?
                    .confirm(storage.get_consensus(height)?, None)
            })
        },
    };
    let protocol = Arc::new(novasymph::EpochProtocol::new(config));
    let main_loop = async {
        loop {
            let confirmed = protocol.next_confirmed().await;
            let height = confirmed.inner().inner_ref().height;
            if let Err(err) = storage
                .apply_block(confirmed.inner().to_block(), confirmed.cproof().clone())
                .await
            {
                log::warn!(
                    "could not apply confirmed block {} from novasymph: {:?}",
                    height,
                    err
                );
            }
        }
    };
    let reset_loop = async {
        loop {
            let latest_known = storage.highest_state();
            let protocol = protocol.clone();
            smol::unblock(move || protocol.reset_genesis(latest_known)).await;
            smol::Timer::after(Duration::from_secs(5)).await;
        }
    };
    main_loop.race(reset_loop).await
}

struct StorageBlockBuilder {
    storage: NodeStorage,
    payout_covhash: Address,
    target_fee_multiplier: u128,
}

impl BlockBuilder<MeshaCas> for StorageBlockBuilder {
    fn build_block(&self, tip: SealedState<MeshaCas>) -> Block {
        let proposer_action = ProposerAction {
            fee_multiplier_delta: if tip.header().fee_multiplier > self.target_fee_multiplier {
                i8::MIN
            } else {
                i8::MAX
            },
            reward_dest: self.payout_covhash,
        };
        let mempool_state = self
            .storage
            .mempool()
            .to_state()
            .seal(Some(proposer_action));
        if mempool_state.header().previous != tip.header().hash() {
            log::warn!(
                "mempool {} doesn't extend from tip {}; building quasiempty block",
                mempool_state.header().height,
                tip.header().height
            );

            let next = tip.next_state().seal(Some(proposer_action));
            next.to_block()
        } else {
            self.storage
                .mempool_mut()
                .rebase(mempool_state.next_state());
            mempool_state.to_block()
        }
    }

    fn hint_next_build(&self, tip: SealedState<MeshaCas>) {
        self.storage.mempool_mut().rebase(tip.next_state());
    }

    fn get_cached_transaction(&self, txhash: TxHash) -> Option<Transaction> {
        self.storage.mempool().lookup_recent_tx(txhash)
    }
}
