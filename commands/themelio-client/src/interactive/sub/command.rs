use serde::{Deserialize, Serialize};
use serde_scan::ScanError;
use std::convert::TryFrom;

#[derive(Eq, PartialEq, Clone, Serialize, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InteractiveSubCommand {
    Faucet(String, String),
    // Deposit(String, String, String, String),
    // Withdraw(String, String, String, String),
    // Swap(String, String),
    SendCoins(String, String, String),
    AddCoins(String),
    ShowBalance,
    Help,
    Exit,
}

impl TryFrom<String> for InteractiveSubCommand {
    type Error = ScanError;

    /// Uses serde scan internally to parse a whitespace delimited string into a command
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let cmd: Result<InteractiveSubCommand, _> = serde_scan::from_str(&value);
        cmd
    }
}
