use serde::{Deserialize, Serialize};
use serde_scan::ScanError;
use std::convert::TryFrom;
use std::fmt;

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShellCommand {
    CreateWallet(String),
    ShowWallets,
    OpenWallet(String, String),
    UseWallet(String, String),
    // TODO: Add in delete wallet
    // DeleteWallet(String),
    Help,
    Exit,
}

impl fmt::Display for ShellCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let x = vec!["hi".to_string()];
        let params: Vec<String> = match self {
            ShellCommand::UseWallet(a, b) => { vec![a.to_string(), b.to_string()]},
            _ => { vec![] }
        };
        // write!(f, "({:?}", params);
        Ok(())
    }
}

impl TryFrom<String> for ShellCommand {
    type Error = ScanError;

    /// Uses serde scan internally to parse a whitespace delimited string into a command
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let cmd: Result<ShellCommand, _> = serde_scan::from_str(&value);
        cmd
    }
}