use std::time::Duration;

use smol::Timer;

use nodeprot::{ValClient, ValClientSnapshot};

/// Contains data for the entire life-cycle of a command being executed.
#[derive(Clone, Debug)]
pub struct ExecutionContext {
    pub host: smol::net::SocketAddr,
    pub network: blkstructs::NetID,
    pub database: std::path::PathBuf,
    pub version: String,
    pub default_sleep_sec: u64,
    // pub default_fee: u128,
}

impl ExecutionContext {
    /// Sync the client by getting the latest snapshot from an execution context.
    pub async fn get_latest_snapshot(&self) -> anyhow::Result<ValClientSnapshot> {
        let client = ValClient::new(self.network, self.host);
        let snapshot = client.snapshot_latest().await?;
        Ok(snapshot)
    }

    /// TODO: change to default
    /// Sleep on current async task for a duration set in seconds.
    pub async fn sleep(&self, sec: u64) -> anyhow::Result<()> {
        let duration = Duration::from_secs(sec);
        Timer::after(duration).await;
        Ok(())
    }

    /// Sleep on current async task for a default duration set in seconds.
    pub async fn sleep_default(&self) -> anyhow::Result<()> {
        let duration = Duration::from_secs(self.default_sleep_sec);
        Timer::after(duration).await;
        Ok(())
    }
}
