use colored::Colorize;
use crate::wallet::data::WalletData;
use tabwriter::TabWriter;

use std::io::prelude::*;
use std::collections::BTreeMap;
use crate::wallet::wallet::Wallet;

pub struct CommandOutput {}

impl CommandOutput {
    /// Display name, secret key and covenant of the wallet.
    pub(crate) async fn print_created_wallet(wallet: Wallet) -> anyhow::Result<()> {
        let mut tw = TabWriter::new(vec![]);
        writeln!(tw, ">> New data:\t{}", wallet.name.bold()).unwrap();
        writeln!(tw, ">> Address:\t{}", wallet.data.my_script.hash().to_addr().yellow()).unwrap();
        writeln!(tw, ">> Secret:\t{}", hex::encode(wallet.sk.0).dimmed()).unwrap();
        eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
        Ok(())
    }

    /// Display all stored wallet wallet addresses by name.
    pub(crate) async fn show_all_wallets(wallets: BTreeMap<String, WalletData>) {
        let mut tw = TabWriter::new(vec![]);
        writeln!(tw, ">> [NAME]\t[ADDRESS]");
        for (name, wallet) in wallets {
            writeln!(tw, ">> {}\t{}", name, wallet.my_script.hash().to_addr());
        }
        tw.flush();
        eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
    }
}