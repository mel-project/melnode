pub mod lib;

use lib::options::ClientOpts;
use lib::dispatcher::ClientDispatcher;

use structopt::StructOpt;

/// Run client with command line options
fn main() {
    let version = env!("CARGO_PKG_VERSION");
    let opts: lib::ClientOpts = ClientOpts::from_args();
    let dispatcher = ClientDispatcher::new(opts, version);
    smolscale::block_on(async move {
        dispatcher.dispatch().await
    });
}
