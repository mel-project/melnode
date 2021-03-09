use std::net::SocketAddr;

use blkstructs::{Block, ConsensusProof, Header, State, Transaction};
use serde::{Deserialize, Serialize};

use tmelcrypt::HashVal;

/// This cancellable async function synchronizes the block state with some other node. If the other node has the *next* block, it is returned; otherwise None is returned.
///
/// Right now we don't have a decent fastsync protocol yet, but that's fine for the testnet.
#[tracing::instrument(skip(get_cached_tx))]
pub async fn sync_state(
    remote: SocketAddr,
    netname: &str,
    starting_height: u64,
    get_cached_tx: impl Fn(HashVal) -> Option<Transaction> + Send + Sync,
) -> anyhow::Result<Vec<(Block, ConsensusProof)>> {
    const BLKSIZE: u64 = 128;
    let exec = smol::Executor::new();
    let tasks = {
        let mut toret = Vec::new();
        for height in starting_height..starting_height + BLKSIZE {
            let task = exec.spawn(get_one_block(remote, netname, height, &get_cached_tx));
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
    remote: SocketAddr,
    netname: &str,
    height: u64,
    get_cached_tx: &(impl Sync + Fn(HashVal) -> Option<Transaction>),
) -> anyhow::Result<(Block, ConsensusProof)> {
    let remote_state: (AbbreviatedBlock, ConsensusProof) = melnet::g_client()
        .request(remote, netname, "get_state", height)
        .await?;
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
    let txx: Vec<Transaction> = melnet::g_client()
        .request(remote, netname, "get_txx", unknown_txhashes)
        .await?;
    all_txx.extend(txx);
    // now we should be able to construct the state
    let new_block = Block {
        header: remote_state.0.header,
        transactions: all_txx.into(),
        proposer_action: None,
    };
    Ok((new_block, remote_state.1))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AbbreviatedBlock {
    pub header: Header,
    pub txhashes: Vec<HashVal>,
}

impl AbbreviatedBlock {
    pub fn from_state(state: &blkstructs::SealedState) -> Self {
        let header = state.header();
        let txhashes: Vec<HashVal> = state
            .inner_ref()
            .transactions
            .val_iter()
            .map(|v| v.hash_nosigs())
            .collect();
        Self { header, txhashes }
    }
}

// TODO: where does this test go?
// async fn test_spam_txx(auditor: Auditor) {
//     let (_, sk) = tmelcrypt::ed25519_keygen();
//     let txx = blkstructs::testing::random_valid_txx(
//         &mut rand::thread_rng(),
//         blkstructs::CoinID {
//             txhash: tmelcrypt::HashVal::default(),
//             index: 0,
//         },
//         blkstructs::CoinData {
//             conshash: blkstructs::melvm::Covenant::always_true().hash(),
//             value: blkstructs::MICRO_CONVERTER * 1000,
//             cointype: blkstructs::COINTYPE_TMEL.to_owned(),
//         },
//         sk,
//         &blkstructs::melvm::Covenant::always_true(),
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
