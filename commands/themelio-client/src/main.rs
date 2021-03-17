use structopt::StructOpt;
use std::path::PathBuf;

#[derive(Debug, StructOpt)]
#[structopt(name = "Themelio Client CLI")]
/// A command line application to interact with Themelio
///
/// This binary supports all features for a Themelio client.
/// End users can create, delete, import, and export wallets.
/// Interacting with the blockchain is done by opening a specific wallet
/// and creating and sending various supported transactions.
pub struct ClientOpts {
    /// IP Address with port used to establish a connection to host
    #[structopt(long)]
    host: smol::net::SocketAddr,

    // File path to database for client wallet storage
    #[structopt(long, short, parse(from_os_str), default_value="/tmp/testnet")]
    database: PathBuf,
}

fn main() {
    let opts: ClientOpts = ClientOpts::from_args();
    smolscale::block_on(run_client(opts.host, opts.database))
}

enum WalletPromptOpt {
    CreateWallet(String),
    ImportWallet(PathBuf),
    ExportWallet(PathBuf),
    ShowWallets,
    OpenWallet(Wallet),
}

struct Wallet {}

pub struct WalletStorage {
    wallets: SledMap<String, WalletData>
}

impl WalletStorage {
    /// Opens a WalletStorage, given a sled database.
    pub fn new(db: sled::Db) -> Self {
        let wallets = SledMap::new(db.open_tree("wallet").unwrap());
        Self {
            wallets
        }
    }
}

enum OpenWalletPromptOpt {

}

async fn run_client(host: smol::net::SocketAddr, database: PathBuf) {
    let prompt = WalletPrompt::new();
    let db: sled::Db = sled::Db::new(database);
    let mut storage = WalletStorage::new(db);

    loop {
        let prompt_result = handle_wallet_prompt(&prompt, &storage).await?;
        // handle res err handling if any here
    }
}

async fn handle_wallet_prompt(prompt: &WalletPrompt, storage: &WalletStorage) -> anyhow::Result<()> {
    let opt: WalletPromptOpt = prompt::handle_input();
    match opt {
        WalletPromptOpt::CreateWallet(name) => {
            let wallet: Wallet = Wallet::new(&name);
            prompt.show_wallet(&wallet);
            storage.save(&name, &wallet)?
        }
        WalletPromptOpt::ShowWallets => {
            let wallets: Vec<Wallet> = storage.load_all()?;
            prompt.show_wallets(&wallets);
        }
        WalletPromptOpt::OpenWallet(wallet) => {
            let prompt_result = handle_open_wallet_prompt(&prompt, &storage).await?;
            // handle res err if any
        }
        // WalletPromptOpt::ImportWallet(_import_path) => {}
        // WalletPromptOpt::ExportWallet(_export_path) => {}
        _ => {}
    }
}

async fn handle_open_wallet_prompt() -> anyhow::Result<()> {
    let prompt = OpenWalletPrompt::new();
    let opt: OpenWalletPromptOpt = prompt::handle_input();

    match opt {}

    //flow pseudo-code
    //     - swap
    //     - input pool, buy/sell, token name, denom, amount
    //     - create tx
    //     - presend (do we sign here?)
    // - send
    //     - query
    //     - print query results
    //     - update storage
    //     - send
    //     - input dest, amount, denom
    //     - create
    //     - presend (do we sign here?) / fee calc?
    //     - send
    //     - query / print query results
    //     - update storage
    //     - receive
    //     - input coin id
    //     - query
    //     - update storage
    //     - deposit / withdraw
    //     - input pool
    //     - input token
    //     - input amount (do we validate?)
    // - create tx
    //     - prespend
    //     - send
    //     - query / print query results
    //     - update storage
    //     - faucet (enable-disable based on mainnet or not)
    // - input receiver address, amount, denom, amount (upper bounded?)
    // - create tx
    //     - presend
    //     - query
    //     - update storage
    //     - balance
    //     - load storage
    //     - print storage balance
    //     - coins
    //     - load storage
    //     - print storage coins
// }
}


