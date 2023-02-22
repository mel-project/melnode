use std::{fmt::Debug, time::Duration};

use futures_util::Future;

/// Retries a function until it works, internally doing exponential backoff
pub async fn autoretry<T, E: Debug, Fut: Future<Output = Result<T, E>>, Fun: FnMut() -> Fut>(
    mut f: Fun,
) -> T {
    let mut sleep_interval = Duration::from_millis(50);
    loop {
        match f().await {
            Ok(val) => return val,
            Err(err) => {
                log::warn!("autoretrying due to {:?}", err);
                smol::Timer::after(sleep_interval).await;
                sleep_interval *= 2;
            }
        }
    }
}
