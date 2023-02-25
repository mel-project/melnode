use std::path::PathBuf;

use clap::Parser;
use melnode::storage::Storage;
use melstf::GenesisConfig;
use melstructs::{Block, ConsensusProof};

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    old: PathBuf,

    #[arg(long)]
    new: PathBuf,
}

fn main() -> anyhow::Result<()> {
    smolscale::block_on(async move {
        let args = Args::parse();
        let storage = Storage::open(args.new, GenesisConfig::std_mainnet()).await?;
        let directory = std::fs::read_dir(args.old)?;
        let mut paths = vec![];
        for file in directory {
            let file = file?;
            paths.push(file.path());
            eprintln!("touching {:?}", file.path());
        }
        paths.sort_unstable();
        let total = paths.len();
        for file in paths {
            let raw_block = smol::fs::read(file).await?;
            let (blk, cproof): (Block, ConsensusProof) = stdcode::deserialize(&raw_block)?;
            eprintln!(
                "[{:.2}%] applying {}/{total}",
                100.0 * blk.header.height.0 as f64 / total as f64,
                blk.header.height
            );
            storage.apply_block(blk, cproof).await?;
        }
        Ok(())
    })
}
