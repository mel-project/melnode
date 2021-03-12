use std::env;
use config::{ConfigError, Config, File, Environment};

#[serde_as]
#[derive(Debug, Deserialize)]
struct Smt {
    #[serde_as(as = "BytesOrString")]
    data_block_hash_key: Vec<u8>,
    #[serde_as(as = "BytesOrString")]
    node_hash_val: Vec<u8>,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    smt: Smt,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let settings_file = config::File::with_name("Settings");
        let mut settings = config::Config::default();
        settings.merge(&settings_file).unwrap();
        settings.try_into()
    }
}

lazy_static! {
    pub static ref SETTINGS: Settings = Settings::new();
}