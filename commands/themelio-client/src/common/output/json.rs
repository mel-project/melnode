use std::collections::BTreeMap;
use std::io::prelude::*;

use colored::Colorize;
use tabwriter::TabWriter;

use blkstructs::{CoinDataHeight, CoinID};

use crate::wallet::data::WalletData;
use crate::wallet::wallet::Wallet;
use crate::common::formatter::OutputFormatter;

pub struct JsonOutputFormatter { }

impl OutputFormatter for JsonOutputFormatter {
    /// Display name, secret key and covenant of the wallet.
    fn wallet(&self, wallet: Wallet) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(&wallet).unwrap();
        eprintln!("{}", json);
        Ok(())
    }

    /// Display all stored wallet wallet addresses by name.
    fn wallet_addresses_by_name(&self, wallets: BTreeMap<String, WalletData>) -> anyhow::Result<()> {
        let mut tw = TabWriter::new(vec![]);
        writeln!(tw, ">> [NAME]\t[ADDRESS]");
        for (name, wallet) in wallets {
            writeln!(tw, ">> {}\t{}", name, wallet.my_script.hash().to_addr());
        }
        tw.flush();
        eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
        Ok(())
    }

    /// Display message showing height and coin id information upon a coin being confimed.
    fn coin_confirmed(&self, coin_data_height: &CoinDataHeight, coin: &CoinID) -> anyhow::Result<()> {
        eprintln!(
            ">>> Coin is confirmed at current height {}",
            coin_data_height.height
        );
        eprintln!(
            ">> CID = {}",
            hex::encode(stdcode::serialize(&coin).unwrap()).bold()
        );
        Ok(())
    }

    /// Display message that coin is not yet confirmed.
    fn coin_pending(&self) -> anyhow::Result<()> {
        eprintln!(">>> Coin is not yet confirmed");
        Ok(())
    }

    /// Display function which displays pending message until a coin is confirmed
    /// at which a confirmed message will be displayed.
    fn check_coin(&self, coin_data_height: &Option<CoinDataHeight>, coin_id: &CoinID) -> anyhow::Result<()> {
        match coin_data_height {
            None => coin_pending().await,
            Some(coin_data_height) => self.coin_confirmed(&coin_data_height, &coin_id).await,
        }
        Ok(())
    }
}
