use crate::storage::Storage;
use anyhow::Context;
use base64::Engine;
use futures_util::stream::{StreamExt, TryStreamExt};
use melprot::NodeRpcClient;
use smol_timeout::TimeoutExt;
use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};
use themelio_structs::{Block, BlockHeight, ConsensusProof};

/// Attempts a sync using the given given node client.
pub async fn attempt_blksync(
    addr: SocketAddr,
    client: &NodeRpcClient,
    storage: &Storage,
) -> anyhow::Result<usize> {
    if std::env::var("MELNODE_OLD_BLKSYNC").is_ok() {
        return attempt_blksync_legacy(addr, client, storage).await;
    }

    let their_highest = client
        .get_summary()
        .timeout(Duration::from_secs(5))
        .await
        .context("timed out getting summary")?
        .context("cannot get their highest block")?
        .height;

    let my_highest = storage.highest_height().await;
    if their_highest <= my_highest {
        return Ok(0);
    }

    let mut num_blocks_applied: usize = 0;
    let my_highest: u64 = my_highest.0 + 1;

    let mut height = BlockHeight(my_highest);
    while height <= their_highest {
        let start = Instant::now();

        log::debug!("gonna get compressed blocks from {addr}...");
        let compressed_blocks = client
            .get_lz4_blocks(height, 500_000)
            .timeout(Duration::from_secs(30))
            .await
            .context("timeout while getting compressed blocks")?
            .context("failed to get compressed blocks")?;
        log::debug!("got compressed blocks!");

        let (blocks, cproofs): (Vec<Block>, Vec<ConsensusProof>) = match compressed_blocks {
            Some(compressed) => {
                // decode base64 first
                let compressed_base64 = base64::engine::general_purpose::STANDARD_NO_PAD
                    .decode(compressed.as_bytes())?;

                // decompress
                let decompressed = lz4_flex::decompress_size_prepended(&compressed_base64)?;

                stdcode::deserialize::<(Vec<Block>, Vec<ConsensusProof>)>(&decompressed)?
            }
            _ => anyhow::bail!("missing block {height}"),
        };

        let mut last_applied_height = height;
        log::info!(
            "fully resolved blocks {}..{} from peer {} in {:.2}ms",
            blocks.first().map(|b| b.header.height).unwrap_or_default(),
            blocks.last().map(|b| b.header.height).unwrap_or_default(),
            addr,
            start.elapsed().as_secs_f64() * 1000.0
        );
        for (block, cproof) in blocks.iter().zip(cproofs) {
            // validate before applying
            if block.header.height != last_applied_height {
                anyhow::bail!("wanted block {}, but got {}", height, block.header.height);
            }

            storage
                .apply_block(block.clone(), cproof)
                .await
                .context("could not apply a resolved block")?;
            num_blocks_applied += 1;

            last_applied_height += BlockHeight(1);
        }

        height += BlockHeight(blocks.len() as u64);
    }

    Ok(num_blocks_applied)
}

/// Attempts a sync using the given given node client, in a legacy fashion.
pub async fn attempt_blksync_legacy(
    addr: SocketAddr,
    client: &NodeRpcClient,
    storage: &Storage,
) -> anyhow::Result<usize> {
    let their_highest = client
        .get_summary()
        .timeout(Duration::from_secs(5))
        .await
        .context("timed out getting summary")?
        .context("cannot get their highest block")?
        .height;
    let my_highest = storage.highest_height().await;
    if their_highest <= my_highest {
        return Ok(0);
    }
    let height_stream = futures_util::stream::iter((my_highest.0..=their_highest.0).skip(1))
        .map(BlockHeight)
        .take(
            std::env::var("THEMELIO_BLKSYNC_BATCH")
                .ok()
                .and_then(|d| d.parse().ok())
                .unwrap_or(1000),
        );
    let lookup_tx = |tx| storage.mempool().lookup_recent_tx(tx);
    let mut result_stream = height_stream
        .map(Ok::<_, anyhow::Error>)
        .try_filter_map(|height| async move {
            Ok(Some(async move {
                let start = Instant::now();

                let (block, cproof): (Block, ConsensusProof) = match client
                    .get_full_block(height, &lookup_tx)
                    .timeout(Duration::from_secs(15))
                    .await
                    .context("timeout")??
                {
                    Some(v) => v,
                    _ => anyhow::bail!("mysteriously missing block {}", height),
                };

                if block.header.height != height {
                    anyhow::bail!("WANTED BLK {}, got {}", height, block.header.height);
                }
                log::trace!(
                    "fully resolved block {} from peer {} in {:.2}ms",
                    block.header.height,
                    addr,
                    start.elapsed().as_secs_f64() * 1000.0
                );
                Ok((block, cproof))
            }))
        })
        .try_buffered(64)
        .boxed();
    let mut toret = 0;
    while let Some(res) = result_stream.try_next().await? {
        let (block, proof): (Block, ConsensusProof) = res;

        storage
            .apply_block(block, proof)
            .await
            .context("could not apply a resolved block")?;
        toret += 1;
    }
    Ok(toret)
}
