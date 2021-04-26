use std::collections::BTreeMap;
use std::io::prelude::*;

use colored::Colorize;
use tabwriter::TabWriter;

use blkstructs::{CoinDataHeight, CoinID};

use crate::utils::formatter::formatter::OutputFormatter;
use crate::wallet::data::WalletData;
use crate::wallet::wallet::ActiveWallet;
use async_trait::async_trait;

pub struct PlainOutputFormatter {}

#[async_trait]
impl OutputFormatter for PlainOutputFormatter {
    /// Display name, secret key and covenant of the wallet.
    async fn wallet(&self, wallet: ActiveWallet) -> anyhow::Result<()> {
        let mut tw = TabWriter::new(vec![]);
        writeln!(tw, ">> New data:\t{}", wallet.name().bold()).unwrap();
        writeln!(
            tw,
            ">> Address:\t{}",
            wallet.data().my_covenant().hash().to_addr().yellow()
        )
        .unwrap();
        writeln!(
            tw,
            ">> Secret:\t{}",
            hex::encode(wallet.secret().0).dimmed()
        )
        .unwrap();
        eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
        Ok(())
    }

    /// Display all stored wallet wallet addresses by name.
    async fn wallet_addresses_by_name(
        &self,
        wallets: BTreeMap<String, WalletData>,
    ) -> anyhow::Result<()> {
        let mut tw = TabWriter::new(vec![]);
        writeln!(tw, ">> [NAME]\t[ADDRESS]").unwrap();
        for (name, wallet) in wallets {
            writeln!(tw, ">> {}\t{}", name, wallet.my_covenant().hash().to_addr()).unwrap();
        }
        tw.flush().unwrap();
        eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
        Ok(())
    }

    /// Display message showing height and coin id information upon a coin being confimed.
    async fn coin_confirmed(
        &self,
        coin_data_height: &CoinDataHeight,
        coin: &CoinID,
    ) -> anyhow::Result<()> {
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
    async fn coin_pending(&self) -> anyhow::Result<()> {
        eprintln!(">>> Coin is not yet confirmed");
        Ok(())
    }

    /// Display function which displays pending message until a coin is confirmed
    /// at which a confirmed message will be displayed.
    async fn check_coin(
        &self,
        coin_data_height: &Option<CoinDataHeight>,
        coin_id: &CoinID,
    ) -> anyhow::Result<()> {
        match coin_data_height {
            None => self.coin_pending().await?,
            Some(coin_data_height) => self.coin_confirmed(&coin_data_height, &coin_id).await?,
        }
        Ok(())
    }
}
