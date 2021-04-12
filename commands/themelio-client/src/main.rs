use crate::dispatcher::ClientDispatcher;

pub mod options;
pub mod shell;
pub mod common;
pub mod error;
pub mod wallet;
pub mod dispatcher;

use options::ClientOpts;

use structopt::StructOpt;

/// Run client with command line options
fn main() {
    let version = env!("CARGO_PKG_VERSION");
    let opts: ClientOpts = ClientOpts::from_args();
    let dispatcher = ClientDispatcher::new(opts, version);
    smolscale::block_on(async move {
        dispatcher.dispatch().await
    });
}
