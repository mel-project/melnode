use std::net::{Ipv4Addr, SocketAddr};

use clap::{Args, Parser, Subcommand};
use melnode::args::StakerConfig;

use stdcode::StdcodeSerializeExt;
use themelio_stf::GenesisConfig;
use themelio_structs::{Address, CoinData, CoinValue, Denom, NetID, StakeDoc, TxHash};
use tmelcrypt::{Ed25519SK, Hashable};

#[derive(Parser)]
struct Command {
    #[command(subcommand)]
    command: Sub,
}

#[derive(Subcommand)]
enum Sub {
    Create(CreateArgs),
}

#[derive(Args)]
struct CreateArgs {
    #[arg(short, long)]
    stake: Vec<CoinValue>,
}

fn main() -> anyhow::Result<()> {
    match Command::parse().command {
        Sub::Create(create) => main_create(create),
    }
}

fn main_create(create: CreateArgs) -> anyhow::Result<()> {
    let secrets: Vec<Ed25519SK> = create.stake.iter().map(|_| Ed25519SK::generate()).collect();
    let genesis_config = GenesisConfig {
        network: NetID::Custom02,
        init_coindata: CoinData {
            covhash: Address(Default::default()),
            value: Default::default(),
            denom: Denom::Mel,
            additional_data: Default::default(),
        },
        stakes: create
            .stake
            .iter()
            .zip(secrets.iter())
            .map(|(amount, key)| {
                (
                    TxHash(key.to_public().stdcode().hash()),
                    StakeDoc {
                        pubkey: key.to_public(),
                        e_start: 0,
                        e_post_end: 1000000,
                        syms_staked: *amount,
                    },
                )
            })
            .collect(),
        init_fee_pool: 0.into(),
        init_fee_multiplier: 100,
    };
    let staker_configs =
        create
            .stake
            .iter()
            .zip(secrets.iter())
            .enumerate()
            .map(|(i, (amount, key))| {
                let addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), (i + 5000) as u16);
                StakerConfig {
                    signing_secret: *key,
                    listen: addr,
                    bootstrap: SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 5000),
                    payout_addr: Address(Default::default()),
                    target_fee_multiplier: 10000,
                }
            });
    for (i, config) in staker_configs.enumerate() {
        let yaml = serde_yaml::to_string(&config)?;
        std::fs::write(format!("staker-{i}.yaml"), yaml.as_bytes())?;
    }
    std::fs::write("genesis.yaml", &serde_yaml::to_vec(&genesis_config)?)?;
    Ok(())
}
