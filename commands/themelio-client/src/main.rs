pub mod lib;

use lib::options::ClientOpts;
use lib::dispatcher::ClientDispatcher;

use structopt::StructOpt;

fn main() {
    smolscale::block_on(async move {
        let dispatcher = ClientDispatcher::new(ClientOpts::from_args(), env!("CARGO_PKG_VERSION"));
        dispatcher.dispatch().await
    });
}
