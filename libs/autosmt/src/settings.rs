use config::ConfigError;
use serde::{Serialize, Deserialize};
use serde_with::{serde_as, BytesOrString};

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Smt {
    #[serde_as(as = "BytesOrString")]
    pub data_block_hash_key: Vec<u8>,
    #[serde_as(as = "BytesOrString")]
    pub node_hash_val: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Settings {
    pub smt: Smt,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let mut settings = config::Config::default();
        settings.merge(config::File::with_name("Settings")).unwrap();
        settings.try_into()
    }
}

lazy_static! {
    pub(crate) static ref SETTINGS: Settings = Settings::new().unwrap();
}