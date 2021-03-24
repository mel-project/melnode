use smol::prelude::*;
use std::pin::Pin;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, MelnetError>;
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

#[derive(Error, Debug)]
pub enum MelnetError {
    #[error("custom error: `{0}`")]
    Custom(String),
    #[error("verb not found")]
    VerbNotFound,
    #[error("internal server error")]
    InternalServerError,
    #[error("network error: `{0}`")]
    Network(std::io::Error),
}

pub const PROTO_VER: u8 = 1;
pub const MAX_MSG_SIZE: u32 = 10 * 1024 * 1024;

pub async fn write_len_bts<T: AsyncWrite + Unpin>(conn: &mut T, rr: &[u8]) -> Result<()> {
    conn.write_all(&(rr.len() as u32).to_be_bytes())
        .await
        .map_err(MelnetError::Network)?;
    conn.write_all(&rr).await.map_err(MelnetError::Network)?;
    conn.flush().await.map_err(MelnetError::Network)?;
    Ok(())
}

pub async fn read_len_bts<T: AsyncRead + Unpin>(conn: &mut T) -> Result<Vec<u8>> {
    // read the response length
    let mut response_len = [0; 4];
    conn.read_exact(&mut response_len)
        .await
        .map_err(MelnetError::Network)?;
    let response_len = u32::from_be_bytes(response_len);
    if response_len > MAX_MSG_SIZE {
        return Err(MelnetError::Network(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "response too big",
        )));
    }
    // read the response
    let mut response_buf = vec![0; response_len as usize];
    conn.read_exact(&mut response_buf)
        .await
        .map_err(MelnetError::Network)?;
    Ok(response_buf)
}
