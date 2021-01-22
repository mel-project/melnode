use crate::services::WalletData;
use crate::{dal::wallet, protocols::NetClient};
use blkstructs::{
    CoinData, CoinDataHeight, CoinID, Header, Transaction, TxKind, COINTYPE_TMEL, MICRO_CONVERTER,
};
use smol::net::SocketAddr;
use tmelcrypt::Ed25519SK;

use autosmt::FullProof;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;

pub struct ActiveWallet {
    client: NetClient,
    sk: Ed25519SK,
    wallet: WalletData,
    conn: Connection,
}

impl ActiveWallet {
    pub fn new(sk: Ed25519SK, wallet: WalletData, remote: SocketAddr, path: &str) -> Self {
        let path = Path::new(path);
        let conn = Connection::open(path).expect("SQLite connection failure");
        wallet::init(&conn).expect("Failed to load wallet");
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
            fee: 1098000,
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
        let (cdh, _proof) = self.client.get_coin(hdr, coin).await?;
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
            .insert_coin(*coin_id, coin_data_height.clone());
        Ok(())
    }

    pub async fn create_tx(
        &mut self,
        dest_addr: &str,
        amount: &str,
        unit: &str,
    ) -> anyhow::Result<(Transaction)> {
        // Create transaction
        let number: u64 = amount.parse()?;
        assert_eq!(unit, "TML");
        let dest_addr = tmelcrypt::HashVal::from_addr(dest_addr)
            .ok_or_else(|| anyhow::anyhow!("can't decode as address"))?;
        let output = CoinData {
            cointype: COINTYPE_TMEL.to_vec(),
            value: number * MICRO_CONVERTER,
            conshash: dest_addr,
        };
        let outputs = vec![output.clone()];
        let (header, _instant) = self.client.last_header().await?;
        let fee_multiplier = header.fee_multiplier;

        let to_send = self.wallet.pre_spend(outputs, fee_multiplier)?.sign_ed25519(self.sk);

        Ok((to_send))
    }

    pub async fn send_tx(
        &mut self,
        to_send: Transaction
    ) -> anyhow::Result<Transaction> {
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
        let _their_coin = CoinID {
            txhash: tx.hash_nosigs(),
            index: 0,
        };
        Ok(self.client.get_coin(header, first_change).await?)
    }

    pub async fn get_balances(&mut self) -> anyhow::Result<HashMap<CoinID, CoinDataHeight>> {
        let mut unspent_coins = HashMap::new();
        for (coin_id, coin_data) in self.wallet.unspent_coins().iter() {
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
