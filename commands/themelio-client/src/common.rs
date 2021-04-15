use blkstructs::NetID;

pub struct ExecutionContext {
    pub host: smol::net::SocketAddr,
    pub network: NetID,
    pub database: std::path::PathBuf,
    pub version: String,
}

/// Handle raw user input using a prompt.
pub async fn read_line(prompt: String) -> anyhow::Result<String> {
    smol::unblock(move || {
        let mut rl = rustyline::Editor::<()>::new();
        Ok(rl.readline(&prompt)?)
    })
    .await
}
