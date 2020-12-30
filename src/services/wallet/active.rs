use crate::services::WalletData;
use blkstructs::{
    CoinData, CoinDataHeight, CoinID, Transaction, TxKind, COINTYPE_TMEL, MICRO_CONVERTER,
};
use smol::net::SocketAddr;
use tmelcrypt::Ed25519SK;

use super::netclient::NetClient;
use autosmt::FullProof;

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

    pub async fn faucet(&mut self, number: &str, unit: &str) -> anyhow::Result<()> {
        let number: u64 = number.parse()?;
        assert_eq!(unit, "TML");
        // create faucet transaction
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

        loop {
            let (hdr, _) = self.client.last_header().await?;
            let (coin_data_height, proof) = self.client.get_coin(hdr, coin).await?;
            match coin_data_height {
                Some(coin_data_height) => {
                    eprintln!(">> Confirmed at height {}!", coin_data_height.height);
                    eprintln!(
                        ">> CID = {}",
                        hex::encode(bincode::serialize(&coin).unwrap()) // .bold()
                    );
                }
                None => eprintln!(">> Not at height {}...", hdr.height),
            }
        }
    }

    // -> Option<(CoinID, u32)>
    pub async fn coin_add(&mut self, coin_id: &str) -> anyhow::Result<()> {
        unimplemented!();
        // display_coin_add(coin_id, height);
        // eprintln!(">> Syncing state...");
        // let header = self.client.last_header().await?.0;
        // let coin_id: CoinID = bincode::deserialize(&hex::decode(coin_id)?)?;
        // let coin_data_height = self.client.get_coin(header, coin_id).await?;
        // match coin_data_height {
        //     None => {
        //         eprintln!(">> No such coin yet at height {}!", header.height);
        //         // continue;
        //     }
        //     Some(coin_data_height) => {
        //         self.wallet.insert_coin(coin_id, coin_data_height.clone());
        //         eprintln!(
        //             ">> Coin found at height {}! Added {} {} to data",
        //             coin_data_height.height,
        //             coin_data_height.coin_data.value,
        //             match coin_data_height.coin_data.cointype.as_slice() {
        //                 COINTYPE_TMEL => "μmel".to_string(),
        //                 val => format!("X-{}", hex::encode(val)),
        //             }
        //         );
        //     }
        // }
    }

    pub async fn tx_send() -> anyhow::Result<()> {
        unimplemented!();
        // let number: u64 = amount.parse()?;
        // assert_eq!(unit, &"TML");
        // let dest_addr = tmelcrypt::HashVal::from_addr(dest_addr)
        //     .ok_or_else(|| anyhow::anyhow!("can't decode as address"))?;
        // let output = CoinData {
        //     cointype: COINTYPE_TMEL.to_vec(),
        //     value: number * MICRO_CONVERTER,
        //     conshash: dest_addr,
        // };
        // let to_send = wallet.pre_spend(vec![output])?.sign_ed25519(*wallet_sk);
        // eprintln!(">> Syncing state...");
        // client.broadcast_tx(to_send.clone()).await?;
        // eprintln!(">> Transaction {:?} broadcast!", to_send.hash_nosigs());
        // eprintln!(">> Waiting for confirmation...");
        // loop {
        //     let header = client.last_header().await?.0;
        //     let first_change = CoinID {
        //         txhash: to_send.hash_nosigs(),
        //         index: 1,
        //     };
        //     let their_coin = CoinID {
        //         txhash: to_send.hash_nosigs(),
        //         index: 0,
        //     };
        //     if let Some(out) = client.get_coin(header, first_change).await? {
        //         eprintln!(">> Confirmed at height {}!", out.height);
        //         eprintln!(
        //             ">> CID = {}",
        //             hex::encode(bincode::serialize(&their_coin).unwrap()).bold()
        //         );
        //         break;
        //     }
        // }
    }

    pub async fn get_balances(&mut self) -> anyhow::Result<()> {
        unimplemented!();
        // writeln!(tw, ">> **** COINS ****")?;
        // writeln!(tw, ">> [CoinID]\t[Height]\t[Amount]\t[CoinType]")?;
        // for (coin_id, coin_data) in wallet.unspent_coins() {
        //     let coin_id = hex::encode(bincode::serialize(coin_id).unwrap());
        //     writeln!(
        //         tw,
        //         ">> {}\t{}\t{}\t{}",
        //         coin_id,
        //         coin_data.height.to_string(),
        //         coin_data.coin_data.value.to_string(),
        //         match coin_data.coin_data.cointype.as_slice() {
        //             COINTYPE_TMEL => "μTML",
        //             _ => "(other)",
        //         },
        //     )?;
        // }
    }
}
