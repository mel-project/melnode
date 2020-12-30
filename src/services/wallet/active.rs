use crate::services::WalletData;
use blkstructs::{
    CoinData, CoinDataHeight, CoinID, Transaction, TxKind, COINTYPE_TMEL, MICRO_CONVERTER,
};
use smol::net::SocketAddr;
use tmelcrypt::Ed25519SK;

use super::netclient::NetClient;
use autosmt::FullProof;
use tabwriter::TabWriter;

pub struct ActiveWallet {
    client: NetClient,
    sk: Ed25519SK,
    wallet: WalletData,
}

impl ActiveWallet {
    pub fn new(sk: Ed25519SK, wallet: WalletData, remote: SocketAddr) -> Self {
        return ActiveWallet {
            sk,
            wallet,
            client: NetClient::new(remote),
        };
    }

    pub async fn faucet(&mut self, number: &str, unit: &str) -> anyhow::Result<(CoinDataHeight)> {
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
        self.client.broadcast_tx(txn).await?;

        // loop until we get coin data height and proof from last header
        loop {
            let (hdr, _) = self.client.last_header().await?;
            let (coin_data_height, proof) = self.client.get_coin(hdr, coin).await?;
            match coin_data_height {
                Some(coin_data_height) => return Ok(coin_data_height),
                None => {
                    eprintln!(">> Not at height {}...", hdr.height);
                    continue;
                }
            }
        }
    }

    /// TODO: move out eprintlns!
    pub async fn coin_get(
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

    pub async fn coin_add(&mut self, coin_id: &CoinID, coin_data_height: &CoinDataHeight) {
        self.wallet
            .insert_coin(coin_id.clone(), coin_data_height.clone());
    }

    pub async fn send_tx(
        &mut self,
        dest_addr: &str,
        amount: &str,
        unit: &str,
    ) -> anyhow::Result<(CoinDataHeight)> {
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
        eprintln!(">> Waiting for confirmation...");
        loop {
            let header = self.client.last_header().await?.0;
            let first_change = CoinID {
                txhash: to_send.hash_nosigs(),
                index: 1,
            };
            let their_coin = CoinID {
                txhash: to_send.hash_nosigs(),
                index: 0,
            };
            let (coin_data_height, full_proof) = self.client.get_coin(header, first_change).await?;
            if let Some(out) = coin_data_height {
                eprintln!(">> Confirmed at height {}!", out.height);
                eprintln!(
                    ">> CID = {}",
                    hex::encode(bincode::serialize(&their_coin).unwrap()) // .bold()
                );
                return Ok(out);
            }
        }
    }

    pub async fn get_balances(&mut self) -> anyhow::Result<()> {
        let mut tw = TabWriter::new(vec![]); // remove this

        // writeln!(tw, ">> **** COINS ****")?;
        // writeln!(tw, ">> [CoinID]\t[Height]\t[Amount]\t[CoinType]")?;
        for (coin_id, coin_data) in self.wallet.unspent_coins() {
            let coin_id = hex::encode(bincode::serialize(coin_id).unwrap());
            // writeln!(
            //     tw,
            //     ">> {}\t{}\t{}\t{}",
            //     coin_id,
            //     coin_data.height.to_string(),
            //     coin_data.coin_data.value.to_string(),
            //     match coin_data.coin_data.cointype.as_slice() {
            //         COINTYPE_TMEL => "μTML",
            //         _ => "(other)",
            //     },
            // )?;
        }
        Ok(())
    }
}
