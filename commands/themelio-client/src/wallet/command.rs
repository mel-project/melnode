use crate::storage::WalletStorage;
use crate::wallet::common::read_line;
use crate::wallet::data::WalletData;
use crate::wallet::open::command::{OpenWalletCommand, OpenWalletCommandDispatcher};
// use blkstructs::melvm::Covenant;
use serde::{Deserialize, Serialize};
// use tabwriter::TabWriter;

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
            WalletCommand::Create(name) => {
                vec![name.to_string()]
            }
            // WalletCommand::Show => {}
            // WalletCommand::Open(_, _) => {}
            // WalletCommand::Use(_, _) => {}
            // WalletCommand::Delete(_) => {}
            // WalletCommand::Help => {}
            // WalletCommand::Exit => {}
        };

        // write!(f, "({}, {})", self.x, self.y)
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

#[derive(Eq, PartialEq, Debug)]
pub struct CreateResult {}

#[derive(Eq, PartialEq, Debug)]
pub struct ShowResult {}

#[derive(Eq, PartialEq, Debug)]
pub struct OpenResult {}

#[derive(Eq, PartialEq, Debug)]
pub struct UseResult {}

#[derive(Eq, PartialEq, Debug)]
pub struct DeleteResult {}

#[derive(Eq, PartialEq, Debug)]
pub enum WalletCommandResult {
    Create(CreateResult),
    Show(ShowResult),
    Open(OpenResult),
    Use(UseResult),
    Delete(DeleteResult),
    Help,
    Exit
}