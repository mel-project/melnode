use serde::{Deserialize, Serialize};
use serde_scan::ScanError;
use std::convert::TryFrom;
use std::fmt;

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WalletCommand {
    Create(String),
    Show,
    Open(String, String),
    Use(String, String),
    Delete(String),
    Help,
    Exit,
}

impl fmt::Display for WalletCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let x = vec!["hi".to_string()];
        let params: Vec<String> = match self {
            WalletCommand::Use(a, b) => { vec![a.to_string(), b.to_string()]},
            _ => { vec![] }
        };
        write!(f, "({:?}", params);
        Ok(())
    }
}

impl TryFrom<String> for WalletCommand {
    type Error = ScanError;

    /// Uses serde scan internally to parse a whitespace delimited string into a command
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let cmd: Result<WalletCommand, _> = serde_scan::from_str(&value);
        cmd
    }
}