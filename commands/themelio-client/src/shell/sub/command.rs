use serde::{Deserialize, Serialize};
use serde_scan::ScanError;
use std::convert::TryFrom;

#[derive(Eq, PartialEq, Clone, Serialize, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OpenWalletCommand {
    Faucet(String, String),
    Deposit,
    Withdraw,
    Swap,
    SendCoins(String, String, String),
    AddCoins(String),
    Balance,
    Help,
    Exit,
}

impl TryFrom<String> for OpenWalletCommand {
    type Error = ScanError;

    /// Uses serde scan internally to parse a whitespace delimited string into a command
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let cmd: Result<OpenWalletCommand, _> = serde_scan::from_str(&value);
        cmd
    }
}
