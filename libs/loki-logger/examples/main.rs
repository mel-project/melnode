#[tokio::main]
async fn main() {
    loki_logger::init("http://loki:3100/loki/api/v1/push", log::LevelFilter::Info).unwrap();

    log::info!("Logged into Loki !");

    // This is here so that the log has time to be sent asynchonously
    #[allow(clippy::empty_loop)]
    loop {}
}
