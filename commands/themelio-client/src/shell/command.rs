use serde::{Deserialize, Serialize};
use serde_scan::ScanError;
use std::convert::TryFrom;

/// Available shell commands with their string arguments.
/// Note that serde scan is used here to convert inline string input arguments for matching.
#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShellCommand {
    CreateWallet(String),
    ShowWallets,
    OpenWallet(String, String),
    // DeleteWallet(String),
    Help,
    Exit,
}

impl TryFrom<String> for ShellCommand {
    type Error = ScanError;

    /// Uses serde scan internally to parse a whitespace delimited string into a command
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let cmd: Result<ShellCommand, _> = serde_scan::from_str(&value);
        cmd
    }
}