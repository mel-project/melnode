use colored::Colorize;
use std::io::Write;
use tabwriter::TabWriter;

use blkstructs::{CoinDataHeight, CoinID, Transaction};
use serde::Serialize;
use std::collections::BTreeMap;

pub trait Printable {
    fn print(&self, w: &mut dyn std::io::Write);
}

// fn test_print(w: &mut dyn Write) {
//     let mut tw = TabWriter::new(vec![]);
//
//     writeln!(tw, ">> test").unwrap();
//
//     let info = String::from_utf8(tw.into_inner().unwrap()).unwrap();
//     write!(w, "{}", &info).unwrap();
// }

#[derive(Serialize, Debug)]
pub struct CreatedWalletInfo {
    pub name: String,
    pub address: String,
    pub secret: String,
}

impl Printable for CreatedWalletInfo {
    fn print(&self, w: &mut dyn Write) {
        let mut tw = TabWriter::new(vec![]);
        let name = self.name.clone();
        let addr = self.address.clone();
        let secret = self.secret.clone();

        writeln!(tw, ">> New data:\t{}", name.bold()).unwrap();
        writeln!(tw, ">> Address:\t{}", addr.yellow()).unwrap();
        writeln!(tw, ">> Secret:\t{}", secret.dimmed()).unwrap();

        let info = String::from_utf8(tw.into_inner().unwrap()).unwrap();
        write!(w, "{}", &info).unwrap();
    }
}

#[derive(Serialize, Debug)]
pub struct FaucetInfo {
    pub coin_id: CoinID,
    pub coin_data_height: CoinDataHeight,
}

impl Printable for FaucetInfo {
    fn print(&self, w: &mut dyn Write) {
        let mut tw = TabWriter::new(vec![]);

        let coin_data_height = self.coin_data_height.clone();
        let coin_id = self.coin_id;
        writeln!(
            tw,
            ">> Transaction confirmed at height: {}",
            coin_data_height.height
        )
        .unwrap();
        writeln!(
            tw,
            ">> (Covenant hash, amount): ({},  {})",
            coin_data_height.coin_data.covhash, coin_data_height.coin_data.value,
        )
        .unwrap();
        writeln!(
            tw,
            ">> Coin ID: = {}",
            hex::encode(stdcode::serialize(&coin_id).unwrap()).bold()
        )
        .unwrap();

        let info = String::from_utf8(tw.into_inner().unwrap()).unwrap();
        write!(w, "{}", &info).unwrap();
    }
}

#[derive(Serialize, Debug)]
pub struct SendCoinsInfo {
    pub coin_id: CoinID,
    pub coin_data_height: CoinDataHeight,
}

impl Printable for SendCoinsInfo {
    fn print(&self, w: &mut dyn Write) {
        let mut tw = TabWriter::new(vec![]);

        let coin_data_height = self.coin_data_height.clone();
        let coin_id = self.coin_id;
        writeln!(
            tw,
            ">> Transaction confirmed at height: {}",
            coin_data_height.height
        )
        .unwrap();
        writeln!(
            tw,
            ">> (Covenant hash, amount): ({},  {})",
            coin_data_height.coin_data.covhash, coin_data_height.coin_data.value,
        )
        .unwrap();
        writeln!(
            tw,
            ">> Coin ID: = {}",
            hex::encode(stdcode::serialize(&coin_id).unwrap()).bold()
        )
        .unwrap();

        let info = String::from_utf8(tw.into_inner().unwrap()).unwrap();
        write!(w, "{}", &info).unwrap();
    }
}

#[derive(Serialize, Debug)]
pub struct DepositInfo;

impl Printable for DepositInfo {
    fn print(&self, w: &mut dyn Write) {}
}

#[derive(Serialize, Debug)]
pub struct WithdrawInfo;

impl Printable for WithdrawInfo {
    fn print(&self, w: &mut dyn Write) {}
}

#[derive(Serialize, Debug)]
pub struct SwapInfo;

impl Printable for SwapInfo {
    fn print(&self, w: &mut dyn Write) {}
}

#[derive(Serialize, Debug)]
pub struct CoinsInfo {
    pub coin_id: CoinID,
    pub coin_data_height: CoinDataHeight,
}

impl Printable for CoinsInfo {
    fn print(&self, w: &mut dyn Write) {}
}

#[derive(Serialize, Debug)]
pub struct BalanceInfo;

impl Printable for BalanceInfo {
    fn print(&self, w: &mut dyn Write) {}
}

#[derive(Serialize, Debug)]
pub struct WalletsInfo {
    pub wallet_addrs_by_name: BTreeMap<String, String>,
}

impl Printable for WalletsInfo {
    fn print(&self, w: &mut dyn Write) {
        let mut tw = TabWriter::new(vec![]);
        let wallet_addrs_by_name = self.wallet_addrs_by_name.clone();

        // Write header
        writeln!(tw, ">> [NAME]\t[ADDRESS]").unwrap();

        // Write rows
        for (name, address) in wallet_addrs_by_name {
            writeln!(tw, ">> {}\t{}", name, address).unwrap();
        }

        let info = String::from_utf8(tw.into_inner().unwrap()).unwrap();
        write!(w, "{}", &info).unwrap();
    }
}
