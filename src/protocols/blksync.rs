use std::net::SocketAddr;

use blkstructs::{Block, ConfirmedState, State, Transaction};
use symphonia::QuorumCert;
use tmelcrypt::HashVal;

use super::AbbreviatedBlock;

/// This cancellable async function synchronizes the block state with some other node. If the other node has the *next* block, it is returned; otherwise None is returned.
///
/// Right now we don't have a decent fastsync protocol yet, but that's fine for the testnet.
pub async fn sync_state(
    remote: SocketAddr,
    netname: &str,
    my_last_state: Option<&State>,
    mut get_cached_tx: impl FnMut(HashVal) -> Option<Transaction>,
) -> anyhow::Result<Option<(Block, QuorumCert)>> {
    let log_tag = format!("sync_state({}, {})", remote, netname);
    // start with get_state
    let next_height = my_last_state.map(|v| v.height + 1).unwrap_or(0);
    let remote_state: (AbbreviatedBlock, QuorumCert) = melnet::g_client()
        .request(remote, netname, "get_state", next_height)
        .await?;
    log::debug!(
        "{}: remote_state with height={}, count={}",
        log_tag,
        remote_state.0.header.height,
        remote_state.0.txhashes.len()
    );
    // now let's check the state
    if remote_state.0.header.height != next_height {
        anyhow::bail!("server responded with the wrong height");
    }
    if remote_state
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
