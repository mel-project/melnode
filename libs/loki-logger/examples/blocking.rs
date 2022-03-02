#[cfg(feature = "blocking")]
fn main() {
    loki_logger::init("http://loki:3100/loki/api/v1/push", log::LevelFilter::Info).unwrap();

    log::info!("Logged into Loki !");
}

#[cfg(not(feature = "blocking"))]
fn main() {
    panic!("This should only be called when blocking is enabled.")
}
