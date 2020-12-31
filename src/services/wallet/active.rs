use crate::dal::wallet;
use crate::services::WalletData;
use blkstructs::{
    CoinData, CoinDataHeight, CoinID, Header, Transaction, TxKind, COINTYPE_TMEL, MICRO_CONVERTER,
};
use smol::net::SocketAddr;
use tmelcrypt::Ed25519SK;

use super::netclient::NetClient;
use autosmt::FullProof;
use rusqlite::{Connection, Error};
use std::collections::HashMap;
use std::path::Path;

pub struct ActiveWallet {
    client: NetClient,
    sk: Ed25519SK,
    wallet: WalletData,
    conn: Connection,
}

impl ActiveWallet {
    pub fn new(sk: Ed25519SK, wallet: WalletData, remote: SocketAddr, path: &String) -> Self {
        let path = Path::new(path);
        let conn = Connection::open(path).expect("SQLite connection failure");
        wallet::init(&conn);
        ActiveWallet {
            sk,
            wallet,
            client: NetClient::new(remote),
            conn,
        }
    }

    pub async fn send_faucet_tx(&mut self, number: &str, unit: &str) -> anyhow::Result<CoinID> {
        // validate input
        let number: u64 = number.parse()?;
        assert_eq!(unit, "TML");
        // create faucet transaction and broadcast it
        let txn = Transaction {
            kind: TxKind::Faucet,
            inputs: vec![],
            outputs: vec![CoinData {
                cointype: COINTYPE_TMEL.to_owned(),
                conshash: self.wallet.my_script.hash(),
                value: number * MICRO_CONVERTER,
            }],
            fee: 0,
            scripts: vec![],
            sigs: vec![],
            data: vec![],
        };

        let coin = CoinID {
            txhash: txn.hash_nosigs(),
            index: 0,
        };

        self.client
            .broadcast_tx(txn)
            .await
            .expect("Error in broadcast_tx");

        Ok(coin)
    }

    pub async fn get_coin_data(
        &mut self,
        coin: CoinID,
    ) -> anyhow::Result<(Option<CoinDataHeight>, Header)> {
        let (hdr, _) = self.client.last_header().await?;
        let (cdh, proof) = self.client.get_coin(hdr, coin).await?;
        Ok((cdh, hdr))
    }

    pub async fn get_coin_data_by_id(
        &mut self,
        coin_id: &str,
    ) -> anyhow::Result<(Option<CoinDataHeight>, CoinID, autosmt::FullProof)> {
        eprintln!(">> Syncing state...");
        let header = self.client.last_header().await?.0;
        eprintln!(">> Retrieving coin at height {}", header.height);
        let coin_id: CoinID = bincode::deserialize(&hex::decode(coin_id)?)?;
        let (coin_data_height, full_proof) = self.client.get_coin(header, coin_id).await?;
        Ok((coin_data_height, coin_id, full_proof))
    }

    pub async fn add_coin(
        &mut self,
        coin_id: &CoinID,
        coin_data_height: &CoinDataHeight,
    ) -> anyhow::Result<()> {
        self.wallet
            .insert_coin(coin_id.clone(), coin_data_height.clone());
        Ok(())
    }

    pub async fn send_tx(
        &mut self,
        dest_addr: &str,
        amount: &str,
        unit: &str,
    ) -> anyhow::Result<Transaction> {
        let number: u64 = amount.parse()?;
        assert_eq!(unit, "TML");
        let dest_addr = tmelcrypt::HashVal::from_addr(dest_addr)
            .ok_or_else(|| anyhow::anyhow!("can't decode as address"))?;
        let output = CoinData {
            cointype: COINTYPE_TMEL.to_vec(),
            value: number * MICRO_CONVERTER,
            conshash: dest_addr,
        };
        let to_send = self.wallet.pre_spend(vec![output])?.sign_ed25519(self.sk);
        eprintln!(">> Syncing state...");
        self.client.broadcast_tx(to_send.clone()).await?;
        eprintln!(">> Transaction {:?} broadcast!", to_send.hash_nosigs());
        self.wallet.spend(to_send.clone())?;
        Ok(to_send)
    }

    pub async fn verify_tx(
        &mut self,
        tx: Transaction,
    ) -> anyhow::Result<(Option<CoinDataHeight>, FullProof)> {
        let header = self.client.last_header().await?.0;
        let first_change = CoinID {
            txhash: tx.hash_nosigs(),
            index: 1,
        };
        let their_coin = CoinID {
            txhash: tx.hash_nosigs(),
            index: 0,
        };
        Ok(self.client.get_coin(header, first_change).await?)
    }

    pub async fn get_balances(&mut self) -> anyhow::Result<HashMap<CoinID, CoinDataHeight>> {
        let mut unspent_coins = HashMap::new();
        for (coin_id, coin_data) in self.wallet.unspent_coins() {
            unspent_coins.insert(coin_id.clone(), coin_data.clone());
        }
        Ok(unspent_coins)
    }

    pub async fn save(&mut self, wallet_name: &str) -> anyhow::Result<()> {
        let encoded_data = bincode::serialize(&self.wallet).unwrap();
        wallet::update_by_name(&self.conn, &wallet_name, &encoded_data)
            .expect("Failed to update wallet");
        Ok(())
    }
}
