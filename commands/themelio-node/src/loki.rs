use std::env;
use std::iter::FromIterator;
use std::collections::HashMap;

use log::LevelFilter;

const LOKI_USERNAME_VARIABLE: &'static str = "LOKI_USERNAME";
const LOKI_PASSWORD_VARIABLE: &'static str = "LOKI_PASSWORD";
const LOKI_PARTIAL_URL: &'static str = "loki.infra.themelio.org/loki/api/v1/push";

pub async fn loki() {
    let loki_username: String = match env::var(LOKI_USERNAME_VARIABLE) {
        Ok(value) => value,
        Err(error) => panic!("The LOKI_USERNAME environment variable must be set: {}", error),
    };

    let loki_password: String = match env::var(LOKI_PASSWORD_VARIABLE) {
        Ok(value) => value,
        Err(error) => panic!("The LOKI_PASSWORD environment variable must be set: {}", error),
    };

    let loki_url: String = format!("https://{}:{}@{}", loki_username, loki_password, LOKI_PARTIAL_URL);

    let labels: HashMap<String, String> = HashMap::from_iter([
        ("service".to_string(), "themelio-node".to_string()),
    ]);

    loki_logger::init_with_labels(
        loki_url,
        LevelFilter::Debug,
        labels,
    ).expect("Could not initialise a connection to loki");

    log::info!("Successfully connected to loki.");
}