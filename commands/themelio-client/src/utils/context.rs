use std::sync::Arc;
use std::time::Duration;

use smol::Timer;

use nodeprot::ValClient;
use storage::SledMap;

use crate::utils::formatter::formatter::OutputFormatter;
use crate::wallet::data::WalletData;

/// Contains data for the entire life-cycle of a command being executed.
///
#[derive(Clone)]
pub struct ExecutionContext {
    pub host: smol::net::SocketAddr,
    pub network: blkstructs::NetID,
    pub database: Arc<SledMap<String, WalletData>>,
    pub client: ValClient,
    pub version: String,
    pub sleep_sec: u64,

    pub formatter: Arc<Box<dyn OutputFormatter + Sync + Send + 'static>>,
}

impl ExecutionContext {
    /// TODO: change to default
    /// Sleep on current async task for a duration set in seconds.
    pub async fn sleep(&self, sec: u64) -> anyhow::Result<()> {
        let duration = Duration::from_secs(sec);
        Timer::after(duration).await;
        Ok(())
    }

    /// Sleep on current async task for a default duration set in seconds.
    pub async fn sleep_default(&self) -> anyhow::Result<()> {
        let duration = Duration::from_secs(self.sleep_sec);
        Timer::after(duration).await;
        Ok(())
    }

    /// Convenience function for getting the fee multiplier.
    pub async fn fee_multiplier(&self) -> anyhow::Result<u128> {
        Ok(self
            .client
            .snapshot()
            .await?
            .current_header()
            .fee_multiplier)
    }
}
