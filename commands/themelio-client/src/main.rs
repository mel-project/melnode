pub mod lib;

use lib::options::ClientOpts;
use lib::dispatcher::ClientDispatcher;

use structopt::StructOpt;

/// Run a single dispatch given command line options.
fn main() {
    smolscale::block_on(async move {
        let version = env!("CARGO_PKG_VERSION");
        let opts: ClientOpts = ClientOpts::from_args();
        let dispatcher = ClientDispatcher::new(opts, version);
        dispatcher.dispatch().await
    });
}
