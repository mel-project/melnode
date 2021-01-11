use std::net::SocketAddr;

use blkstructs::{Block, Header, State, Transaction};
use serde::{Deserialize, Serialize};
use symphonia::QuorumCert;
use tmelcrypt::HashVal;

/// This cancellable async function synchronizes the block state with some other node. If the other node has the *next* block, it is returned; otherwise None is returned.
///
/// Right now we don't have a decent fastsync protocol yet, but that's fine for the testnet.
#[tracing::instrument(skip(get_cached_tx, my_last_state))]
pub async fn sync_state(
    remote: SocketAddr,
    netname: &str,
    my_last_state: Option<&State>,
    mut get_cached_tx: impl FnMut(HashVal) -> Option<Transaction>,
) -> anyhow::Result<Option<(Block, QuorumCert)>> {
    let log_tag = format!("sync_state({}, {})", remote, netname);
    // start with get_state
    let next_height = my_last_state.map(|v| v.height + 1).unwrap_or(0);
    log::trace!("next_height = {}", next_height);
    let remote_state: (AbbreviatedBlock, QuorumCert) = melnet::g_client()
        .request(remote, netname, "get_state", next_height)
        .await?;
    log::trace!(
        "{}: remote_state with height={}, count={}",
        log_tag,
        remote_state.0.header.height,
        remote_state.0.txhashes.len()
    );
    // now let's check the state
    if remote_state.0.header.height != next_height {
        anyhow::bail!("server responded with the wrong height");
    }
    if !remote_state
        .0
        .header
        .validate_cproof(&remote_state.1, my_last_state)
    {
        anyhow::bail!("header didn't pass validation")
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
        transactions: all_txx,
    };
    Ok(Some((new_block, remote_state.1)))
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
//             conshash: blkstructs::melscript::Script::always_true().hash(),
//             value: blkstructs::MICRO_CONVERTER * 1000,
//             cointype: blkstructs::COINTYPE_TMEL.to_owned(),
//         },
//         sk,
//         &blkstructs::melscript::Script::always_true(),
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
