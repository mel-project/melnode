use crate::storage::WalletStorage;
use crate::wallet::common::read_line;
use crate::wallet::data::WalletData;
use crate::wallet::open::command::{OpenWalletCommand, OpenWalletCommandHandler};
use blkstructs::melvm::Covenant;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use tabwriter::TabWriter;

use serde_scan::ScanError;
use std::convert::{TryFrom, TryInto};
use std::io::prelude::*;

#[derive(Eq, PartialEq, Debug, Serialize, Serialize, Deserialize)]
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

impl TryFrom<String> for WalletCommand {
    type Error = ScanError;

    /// Uses serde scan internally to parse a whitespace delimited string into a command
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let cmd: Result<WalletCommand, _> = serde_scan::from_str(&value);
        cmd
    }
}

pub struct CreateResult {}

pub enum WalletCommandResult {
    Create(CreateResult),
    Show(ShowResult),
    Open(OpenResult),
    Use(UseResult),
    Delete(DeleteResult),
    Help,
    Exit
}