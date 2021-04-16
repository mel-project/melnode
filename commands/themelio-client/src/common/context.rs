use nodeprot::{ValClient, ValClientSnapshot};

use smol::Timer;
use std::time::Duration;

/// Contains data for the entire life-cycle of a command being executed.
#[derive(Clone, Debug)]
pub struct ExecutionContext {
    pub host: smol::net::SocketAddr,
    pub network: blkstructs::NetID,
    pub database: std::path::PathBuf,
    pub version: String,
}

impl ExecutionContext {
    /// Sync the client by getting the latest snapshot from an execution context.
    pub async fn get_latest_snapshot(&self) -> anyhow::Result<ValClientSnapshot> {
        let client = ValClient::new(self.network, self.host);
        let snapshot = client.snapshot_latest().await?;
        Ok(snapshot)
    }

    /// Sleep on current async task for a duration set in seconds.
    pub async fn sleep(&self, sec: u64) -> anyhow::Result<()> {
        let duration = Duration::from_secs(sec);
        Timer::after(duration).await;
        Ok(())
    }
}
