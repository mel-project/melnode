use std::collections::BTreeMap;
use std::io::prelude::*;

use colored::Colorize;
use tabwriter::TabWriter;

use blkstructs::{CoinDataHeight, CoinID};

use crate::wallet::data::WalletData;
use crate::wallet::wallet::Wallet;

/// Display name, secret key and covenant of the wallet.
pub(crate) async fn wallet(wallet: Wallet) -> anyhow::Result<()> {
    let mut tw = TabWriter::new(vec![]);
    writeln!(tw, ">> New data:\t{}", wallet.name.bold()).unwrap();
    writeln!(
        tw,
        ">> Address:\t{}",
        wallet.data.my_script.hash().to_addr().yellow()
    )
    .unwrap();
    writeln!(tw, ">> Secret:\t{}", hex::encode(wallet.sk.0).dimmed()).unwrap();
    eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
    Ok(())
}

/// Display all stored wallet wallet addresses by name.
pub(crate) async fn wallet_addresses_by_name(wallets: BTreeMap<String, WalletData>) {
    let mut tw = TabWriter::new(vec![]);
    writeln!(tw, ">> [NAME]\t[ADDRESS]");
    for (name, wallet) in wallets {
        writeln!(tw, ">> {}\t{}", name, wallet.my_script.hash().to_addr());
    }
    tw.flush();
    eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
}

/// Display message showing height and coin id information upon a coin being confimed.
pub(crate) async fn coin_confirmed(coin_data_height: &CoinDataHeight, coin: &CoinID) {
    eprintln!(
        ">>> Coin is confirmed at current height {}",
        coin_data_height.height
    );
    eprintln!(
        ">> CID = {}",
        hex::encode(stdcode::serialize(&coin).unwrap()).bold()
    );
}

/// Display message that coin is not yet confirmed.
pub(crate) async fn coin_pending() {
    eprintln!(">>> Coin is not yet confirmed");
}

/// Display function which displays pending message until a coin is confirmed
/// at which a confirmed message will be displayed.
pub(crate) async fn check_coin(coin_data_height: &Option<CoinDataHeight>, coin_id: &CoinID) {
    match coin_data_height {
        None => coin_pending().await,
        Some(coin_data_height) => coin_confirmed(&coin_data_height, &coin_id).await,
    }
}
