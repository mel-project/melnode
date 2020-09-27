#![feature(try_blocks)]
mod auditor;
pub use auditor::*;
use client::{Client, Wallet};
use tabwriter::TabWriter;
mod stakeholder;
use blkstructs::{
    melscript, CoinData, CoinID, Transaction, TxKind, COINTYPE_TMEL, MICRO_CONVERTER,
};
pub use stakeholder::*;
mod common;
use common::*;
use std::{convert::TryInto, io::Write};
mod storage;
use async_net::TcpListener;
use colored::*;
use parking_lot::RwLock;
use std::time::Duration;
use std::{collections::HashMap, sync::Arc};
use std::{
    // io::BufRead,
    net::{SocketAddr, ToSocketAddrs},
};
pub use storage::*;
mod client;
use structopt::StructOpt;
mod client_protocol;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, StructOpt)]
pub enum Config {
    /// Runs a network node (auditor/stakeholder) that serves clients and other nodes.
    Node(NodeConfig),
    /// Runs a thin client that connects to other nodes.
    AnetClient(AnetClientConfig),
}

#[derive(Debug, StructOpt)]
pub struct NodeConfig {
    /// Listen address
    #[structopt(long, default_value = "0.0.0.0:0")]
    listen: SocketAddr,

    /// Bootstrap addresses
    #[structopt(long)]
    bootstrap: Vec<SocketAddr>,

    /// Test spam
    #[structopt(long)]
    test_spam: bool,

    /// Database path
    #[structopt(long, default_value = "/tmp/testnet")]
    database: String,

    /// Testnet type
    #[structopt(long)]
    test_stakeholder: Option<usize>,
}

#[derive(Debug, StructOpt)]
pub struct AnetClientConfig {
    /// Address for bootstrapping into the network
    #[structopt(long, default_value = "35.225.14.194:18888")]
    bootstrap: SocketAddr,
}

pub async fn run_main(opt: Config) {
    match opt {
        Config::Node(cfg) => run_node(cfg).await,
        Config::AnetClient(cfg) => run_anet_client(cfg).await,
    }
}

/// Runs the main function for a node.
async fn run_node(opt: NodeConfig) {
    let _ = std::fs::create_dir_all(&opt.database);

    log::info!("themelio-core v{} initializing...", VERSION);
    log::info!("bootstrapping with {:?}", opt.bootstrap);
    let listener = TcpListener::bind(opt.listen).await.unwrap();
    let storage = Arc::new(RwLock::new(Storage::open_testnet(&opt.database).unwrap()));
    let auditor = Auditor::new(listener, storage.clone(), &opt.bootstrap)
        .await
        .unwrap();
    if opt.test_spam {
        smol::spawn(test_spam_txx(auditor.clone())).detach();
    }
    if let Some(sh_no) = opt.test_stakeholder {
        smol::spawn(test_stakeholder(sh_no, auditor.clone(), storage.clone())).detach();
    }

    // Storage syncer
    loop {
        Timer::after(Duration::from_secs(600)).await;
        {
            let storage = storage.clone();
            smol::unblock(move || storage.write().sync()).await;
        }
    }
}

async fn read_line(prompt: String) -> anyhow::Result<String> {
    smol::unblock(move || {
        let mut rl = rustyline::Editor::<()>::new();
        Ok(rl.readline(&prompt)?)
    })
    .await
}

/// Runs the alphanet client
async fn run_anet_client(cfg: AnetClientConfig) {
    let mut prompt_stack: Vec<String> = vec![format!("v{}", VERSION).green().to_string()];

    // wallets
    let mut wallets: HashMap<String, Wallet> = HashMap::new();
    let mut current_wallet: Option<(String, tmelcrypt::Ed25519SK)> = None;
    let mut client = Client::new(cfg.bootstrap);

    loop {
        let prompt = format!("[anet client {}]% ", prompt_stack.join(" "));
        let res: anyhow::Result<()> = try {
            let input = read_line(prompt).await.unwrap();
            let mut tw = TabWriter::new(vec![]);
            // wallet mode
            if let Some((wallet_name, wallet_sk)) = &mut current_wallet {
                let wallet = wallets.get_mut(wallet_name).unwrap();
                match input.split(' ').collect::<Vec<_>>().as_slice() {
                    ["faucet", number, unit] => {
                        let number: u64 = number.parse()?;
                        assert_eq!(unit, &"TML");
                        // create faucet transaction
                        let txn = Transaction {
                            kind: TxKind::Faucet,
                            inputs: vec![],
                            outputs: vec![CoinData {
                                cointype: COINTYPE_TMEL.to_owned(),
                                conshash: wallet.my_script.hash(),
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
                        client.broadcast_tx(txn).await?;
                        eprintln!(
                            ">> Faucet transaction for {} mels broadcast!",
                            number.to_string().bold()
                        );
                        eprintln!(">> Waiting for confirmation...");
                        loop {
                            let (hdr, _) = client.last_header().await?;
                            match client.get_coin(hdr, coin).await? {
                                Some(lala) => {
                                    eprintln!(">> Confirmed at height {}!", lala.height);
                                    eprintln!(
                                        ">> CID = {}",
                                        hex::encode(rlp::encode(&coin)).bold()
                                    );
                                    break;
                                }
                                None => eprintln!(">> Not at height {}...", hdr.height),
                            }
                        }
                    }
                    ["coin-add", coin_id] => {
                        eprintln!(">> Syncing state...");
                        let header = client.last_header().await?.0;
                        let coin_id: CoinID = rlp::decode(&hex::decode(coin_id)?)?;
                        let coin_data_height = client.get_coin(header, coin_id).await?;
                        match coin_data_height {
                            None => {
                                eprintln!(">> No such coin yet at height {}!", header.height);
                                continue;
                            }
                            Some(coin_data_height) => {
                                wallet.insert_coin(coin_id, coin_data_height.clone());
                                eprintln!(
                                    ">> Coin found at height {}! Added {} {} to wallet",
                                    coin_data_height.height,
                                    coin_data_height.coin_data.value,
                                    match coin_data_height.coin_data.cointype.as_slice() {
                                        COINTYPE_TMEL => "μmel".to_string(),
                                        val => format!("X-{}", hex::encode(val)),
                                    }
                                );
                            }
                        }
                    }
                    ["tx-send", dest_addr, amount, unit] => {
                        let number: u64 = amount.parse()?;
                        assert_eq!(unit, &"TML");
                        let dest_addr = tmelcrypt::HashVal::from_addr(dest_addr)
                            .ok_or_else(|| anyhow::anyhow!("can't decode as address"))?;
                        let output = CoinData {
                            cointype: COINTYPE_TMEL.to_vec(),
                            value: number * MICRO_CONVERTER,
                            conshash: dest_addr,
                        };
                        let to_send = wallet.pre_spend(vec![output])?.sign_ed25519(*wallet_sk);
                        eprintln!(">> Syncing state...");
                        client.broadcast_tx(to_send.clone()).await?;
                        eprintln!(">> Transaction {:?} broadcast!", to_send.hash_nosigs());
                        eprintln!(">> Waiting for confirmation...");
                        loop {
                            let header = client.last_header().await?.0;
                            let first_change = CoinID {
                                txhash: to_send.hash_nosigs(),
                                index: 1,
                            };
                            let their_coin = CoinID {
                                txhash: to_send.hash_nosigs(),
                                index: 0,
                            };
                            if let Some(out) = client.get_coin(header, first_change).await? {
                                eprintln!(">> Confirmed at height {}!", out.height);
                                eprintln!(
                                    ">> CID = {}",
                                    hex::encode(rlp::encode(&their_coin)).bold()
                                );
                                break;
                            }
                        }
                    }
                    ["balances", ] => {
                        writeln!(tw, ">> **** COINS ****")?;
                        writeln!(tw, ">> [CoinID]\t[Height]\t[Amount]\t[CoinType]")?;
                        for (coin_id, coin_data) in wallet.unspent_coins() {
                            let coin_id = hex::encode(rlp::encode(coin_id));
                            writeln!(
                                tw,
                                ">> {}\t{}\t{}\t{}",
                                coin_id,
                                coin_data.height.to_string(),
                                coin_data.coin_data.value.to_string(),
                                match coin_data.coin_data.cointype.as_slice() {
                                    COINTYPE_TMEL => "μTML",
                                    _ => "(other)",
                                },
                            )?;
                        }
                    }
                    ["exit", ] => {
                        prompt_stack.pop();
                        current_wallet = None;
                    }
                    _ => Err(anyhow::anyhow!("no such command"))?,
                }
            } else {
                match input.split(' ').collect::<Vec<_>>().as_slice() {
                    &["wallet-new", wallet_name] => {
                        if wallets.get(&wallet_name.to_string()).is_some() {
                            eprintln!(">> {}: wallet already exists", "ERROR".red().bold());
                            continue;
                        }
                        let (pk, sk) = tmelcrypt::ed25519_keygen();
                        let script = melscript::Script::std_ed25519_pk(pk);
                        wallets.insert(wallet_name.to_string(), Wallet::new(script.clone()));
                        writeln!(tw, ">> New wallet:\t{}", wallet_name.bold()).unwrap();
                        writeln!(tw, ">> Address:\t{}", script.hash().to_addr().yellow()).unwrap();
                        writeln!(tw, ">> Secret:\t{}", hex::encode(sk.0).dimmed()).unwrap();
                        tw.flush().unwrap();
                    }
                    &["wallet-unlock", wallet_name, wallet_secret] => {
                        if let Some(wallet) = wallets.get(&wallet_name.to_string()) {
                            let wallet_secret = hex::decode(wallet_secret)?;
                            let wallet_secret =
                                tmelcrypt::Ed25519SK(wallet_secret.as_slice().try_into()?);
                            if melscript::Script::std_ed25519_pk(wallet_secret.to_public())
                                != wallet.my_script
                            {
                                Err(anyhow::anyhow!(
                                    "unlocking failed, make sure you have the right secret!"
                                ))?;
                            }
                            current_wallet = Some((wallet_name.to_string(), wallet_secret));
                            prompt_stack.push(format!("({})", wallet_name).yellow().to_string());
                        }
                    }
                    &["wallet-list"] => {
                        writeln!(tw, ">> [NAME]\t[ADDRESS]")?;
                        for (name, wallet) in wallets.iter() {
                            writeln!(tw, ">> {}\t{}", name, wallet.my_script.hash().to_addr())?;
                        }
                    }
                    other => {
                        eprintln!("no such command: {:?}", other);
                        continue;
                    }
                }
            }
            tw.flush()?;
            eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
        };
        if let Err(err) = res {
            eprintln!(">> {}: {}", "ERROR".red().bold(), err);
        }
    }
}

async fn test_stakeholder(sh_no: usize, auditor: Auditor, storage: Arc<RwLock<Storage>>) {
    log::info!("testnet stakeholder {}", sh_no);
    let socket_addr = "0.0.0.0:0".to_socket_addrs().unwrap().next().unwrap();
    let _actor = Stakeholder::new(
        socket_addr,
        auditor,
        storage,
        if sh_no == 0 {
            insecure_testnet_keygen(sh_no).1
        } else {
            tmelcrypt::ed25519_keygen().1
        },
    )
    .await
    .unwrap();
    // block forever now
    loop {
        Timer::after(Duration::from_secs(10000000)).await;
    }
}

async fn test_spam_txx(auditor: Auditor) {
    let (_, sk) = tmelcrypt::ed25519_keygen();
    let txx = blkstructs::testing::random_valid_txx(
        &mut rand::thread_rng(),
        blkstructs::CoinID {
            txhash: tmelcrypt::HashVal::default(),
            index: 0,
        },
        blkstructs::CoinData {
            conshash: blkstructs::melscript::Script::always_true().hash(),
            value: blkstructs::MICRO_CONVERTER * 1000,
            cointype: blkstructs::COINTYPE_TMEL.to_owned(),
        },
        sk,
        &blkstructs::melscript::Script::always_true(),
    );
    log::info!("starting spamming with {} txx", txx.len());
    //let txx = &txx[1..];
    for tx in txx {
        Timer::after(Duration::from_millis(1000)).await;
        auditor
            .send_ret(|s| AuditorMsg::SendTx(tx, s))
            .await
            .unwrap();
    }
}
