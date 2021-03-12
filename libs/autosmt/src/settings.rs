use std::env;
use config::{ConfigError, Config, File, Environment};

#[serde_as]
#[derive(Debug, Deserialize)]
pub(crate) struct Smt {
    #[serde_as(as = "BytesOrString")]
    pub data_block_hash_key: Vec<u8>,
    #[serde_as(as = "BytesOrString")]
    pub node_hash_val: Vec<u8>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Settings {
    pub smt: Smt,
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