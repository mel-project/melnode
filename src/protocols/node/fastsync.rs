use async_compression::futures::{bufread::GzipDecoder, write::GzipEncoder};
use autosmt::DBManager;
use blkstructs::{FastSyncDecoder, FastSyncEncoder, Header, SealedState};
use smol::prelude::*;
use smol::{io::BufReader, net::TcpStream};

/// Streams a fastsync to the other end.
pub async fn send_fastsync(state: SealedState, conn: TcpStream) -> anyhow::Result<()> {
    let mut encoder = FastSyncEncoder::new(state);
    let mut conn = GzipEncoder::new(conn);
    while let Some(chunk) = encoder.next_chunk() {
        let chunk = stdcode::serialize(&chunk)?;
        let len = (chunk.len() as u32).to_be_bytes();
        conn.write(&len).await?;
        conn.write(&chunk).await?;
    }
    conn.flush().await?;
    Ok(())
}

/// Receives a fastsync stream.
pub async fn recv_fastsync(
    dbm: DBManager,
    header: Header,
    conn: TcpStream,
) -> anyhow::Result<SealedState> {
    let mut conn = GzipDecoder::new(BufReader::new(conn));
    let mut decoder = FastSyncDecoder::new(header, dbm);
    let mut len_buf = [0u8; 4];
    loop {
        conn.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf);
        if len > 1024 * 1024 * 10 {
            anyhow::bail!("chunk too big")
        }
        let mut buffer = vec![0u8; len as usize];
        conn.read_exact(&mut buffer).await?;
        let chunk = stdcode::deserialize(&buffer)?;
        if let Some(result) = decoder.process_chunk(chunk)? {
            return Ok(result);
        }
    }
}
