use std::{collections::HashMap, iter::FromIterator};

#[tokio::main]
async fn main() {
    let initial_labels = HashMap::from_iter([
        ("application".to_string(), "loki_logger".to_string()),
        ("environment".to_string(), "development".to_string()),
    ]);

    loki_logger::init_with_labels(
        "http://loki:3100/loki/api/v1/push",
        log::LevelFilter::Info,
        initial_labels,
    )
    .unwrap();

    log::info!("Logged into Loki !");

    // This is here so that the log has time to be sent asynchonously
    #[allow(clippy::empty_loop)]
    loop {}
}
