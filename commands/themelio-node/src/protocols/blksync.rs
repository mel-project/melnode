use std::{net::SocketAddr, time::Duration};

use anyhow::Context;
use futures_util::stream::FuturesUnordered;
use nodeprot::{AbbreviatedBlock, NodeClient};
use smol::{lock::Semaphore, prelude::*};
use smol_timeout::TimeoutExt;
use themelio_stf::{Block, ConsensusProof, NetID, Transaction, TxHash};

/// This cancellable async function synchronizes the block state with some other node. If the other node has the next few blocks, those are returned.
#[tracing::instrument(skip(get_cached_tx))]
pub async fn sync_state(
    netid: NetID,
    remote: SocketAddr,
    starting_height: u64,
    get_cached_tx: impl Fn(TxHash) -> Option<Transaction> + Send + Sync,
) -> anyhow::Result<Vec<(Block, ConsensusProof)>> {
    const BLKSIZE: u64 = 64;
    let exec = smol::Executor::new();
    let tasks = {
        let mut toret = Vec::new();
        for height in starting_height..starting_height + BLKSIZE {
            let task = exec.spawn(get_one_block(netid, remote, height, &get_cached_tx));
            toret.push(task);
        }
        toret
    };
    exec.run(async move {
        let mut toret = Vec::new();
        for (i, task) in tasks.into_iter().enumerate() {
            if i == 0 {
                toret.push(task.await?)
            } else if let Ok(res) = task.await {
                toret.push(res);
            } else {
                break;
            }
        }
        Ok(toret)
    })
    .await
}

/// Obtains *one* block
async fn get_one_block(
    netid: NetID,
    remote: SocketAddr,
    height: u64,
    get_cached_tx: &(impl Sync + Fn(TxHash) -> Option<Transaction>),
) -> anyhow::Result<(Block, ConsensusProof)> {
    log::trace!("get_one_block({})", height);
    let client = NodeClient::new(netid, remote);
    let remote_state: (AbbreviatedBlock, ConsensusProof) = client
        .get_abbr_block(height)
        .timeout(Duration::from_secs(5))
        .await
        .context("timed out")??;
    // now let's check the state
    if remote_state.0.header.height != height {
        anyhow::bail!("server responded with the wrong height");
    }
    // now we get all relevant transactions.
    let mut all_txx = Vec::new();
    let mut unknown_txhashes = Vec::new();
    for txh in remote_state.0.txhashes {
        if let Some(tx) = get_cached_tx(txh) {
            all_txx.push(tx);
        } else {
            unknown_txhashes.push(txh);
        }
    }
    let mut get_tx_tasks = FuturesUnordered::new();
    for txh in unknown_txhashes.iter() {
        let semaph = Semaphore::new(20);
        let client = &client;
        get_tx_tasks.push(async move {
            let _guard = semaph.acquire().await;
            let (tx_content, _proof) = client
                .get_smt_branch(
                    height,
                    nodeprot::Substate::Transactions,
                    tmelcrypt::hash_single(&stdcode::serialize(&txh).unwrap()),
                )
                .timeout(Duration::from_secs(5))
                .await
                .context("timeout")??;
            // TODO check?
            Ok::<_, anyhow::Error>(stdcode::deserialize(&tx_content)?)
        });
    }
    while let Some(res) = get_tx_tasks.next().await {
        all_txx.push(res?);
        log::debug!(
            "loaded {}/{} txx for block {}",
            all_txx.len(),
            unknown_txhashes.len(),
            height,
        );
    }
    // now we should be able to construct the state
    let new_block = Block {
        header: remote_state.0.header,
        transactions: all_txx.into(),
        proposer_action: remote_state.0.proposer_action,
    };
    Ok((new_block, remote_state.1))
}

// TODO: where does this test go?
// async fn test_spam_txx(auditor: Auditor) {
//     let (_, sk) = tmelcrypt::ed25519_keygen();
//     let txx = themelio_stf::testing::random_valid_txx(
//         &mut rand::thread_rng(),
//         themelio_stf::CoinID {
//             txhash: tmelcrypt::HashVal::default(),
//             index: 0,
//         },
//         themelio_stf::CoinData {
//             conshash: themelio_stf::melvm::Covenant::always_true().hash(),
//             value: themelio_stf::MICRO_CONVERTER * 1000,
//             cointype: themelio_stf::COINTYPE_TMEL.to_owned(),
//         },
//         sk,
//         &themelio_stf::melvm::Covenant::always_true(),
//     );
//     log::info!("starting spamming with {} txx", txx.len());
//     //let txx = &txx[1..];
//     for tx in txx {
//         Timer::after(Duration::from_millis(1000)).await;
//         auditor
//             .send_ret(|s| AuditorMsg::SendTx(tx, s))
//             .await
//             .unwrap();
//     }
// }
