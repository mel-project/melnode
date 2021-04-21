use std::convert::TryFrom;

use serde::{Deserialize, Serialize};
use serde_scan::ScanError;

type Amount = String;
type Denom = String;
type Dest = String;
type CoinIdentifier = String;

#[derive(Eq, PartialEq, Clone, Serialize, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubShellCommand {
    Faucet(Amount, Denom),
    // Deposit(String, String, String, String),
    // Withdraw(String, String, String, String),
    // Swap(String, String),
    SendCoins(Dest, Amount, Denom),
    AddCoins(CoinIdentifier),
    ShowBalance,
    Help,
    Exit,
}

impl TryFrom<String> for SubShellCommand {
    type Error = ScanError;

    /// Uses serde scan internally to parse a whitespace delimited string into a command
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let cmd: Result<SubShellCommand, _> = serde_scan::from_str(&value);
        cmd
    }
}
