use colored::Colorize;
use tabwriter::TabWriter;
use tmelcrypt::Ed25519SK;

use std::io::prelude::*;
use std::collections::BTreeMap;
use crate::wallet::data::WalletData;

pub struct ClientOutput {

}

impl ClientOutput {
    /// Display name, secret key and covenant of the shell
    pub(crate) async fn wallet(name: &str, sk: Ed25519SK, wallet_data: &WalletData) -> anyhow::Result<()> {
        // Display contents of keypair and address from covenant
        let mut tw = TabWriter::new(vec![]);
        writeln!(tw, ">> New data:\t{}", name.bold()).unwrap();
        writeln!(tw, ">> Address:\t{}", wallet_data.my_script.hash().to_addr().yellow()).unwrap();
        writeln!(tw, ">> Secret:\t{}", hex::encode(sk.0).dimmed()).unwrap();
        eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
        Ok(())
    }

    pub(crate) async fn wallets(wallets: BTreeMap<String, WalletData>) {
        let mut tw = TabWriter::new(vec![]);
        writeln!(tw, ">> [NAME]\t[ADDRESS]");
        for (name, wallet) in wallets {
            writeln!(tw, ">> {}\t{}", name, wallet.my_script.hash().to_addr());
        }
        tw.flush();
        eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
    }
}