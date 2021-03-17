use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct ClientOpts {
    // /// Listen address
    // #[structopt(long)]
    // listen: SocketAddr,
    //
    // /// Bootstrap addresses // env file?
    // #[structopt(long)]
    // bootstrap: Vec<SocketAddr>,
    //
    // /// Test spam
    // #[structopt(long)]
    // test_spam: bool,
    //
    // /// Database path
    // #[structopt(long, default_value = "/tmp/testnet")]
    // database: String,
    //
    // /// Testnet type
    // #[structopt(long)]
    // test_stakeholder: Option<usize>,
    //
    // /// Listen address for the staker network.
    // #[structopt(long)]
    // listen_staker: Option<SocketAddr>,
}

#[derive(Debug, StructOpt)]
pub struct ClientStorage {

}

fn main() {
    let opts = ClientOpts::from_args();
    let storage = ClientState::load();
    smolscale::block_on(run_client(opts))
}

async fn run_client(opts, storage) {


    // try to connect to nodes, if it fails, sleep, try again
    //
    // - import / create
    //     - export
    //     - list
    //     - create
    //     - generate 24 key pneumonic BIP39 or 34?
    //     - open
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

}

// fn main() {
//     // LogTracer::init().unwrap();
//     let log_conf = std::env::var("RUST_LOG").unwrap_or_else(|_| "themelio_node=debug,warn".into());
//     std::env::set_var("RUST_LOG", log_conf);
//     tracing_subscriber::fmt::init();
//     // env_logger::Builder::from_env("THEMELIO_LOG")
//     //     .parse_filters("themelio_core")
//     //     .init();
//     let opts = NodeConfig::from_args();
//     smolscale::block_on(tasks::run_node(opts))
// }
