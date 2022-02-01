mod blockgraph;
mod cstate;
mod msg;
mod protocol;
use once_cell::sync::Lazy;
pub use protocol::*;

/// Crate-local executor to prevent CPU spikes (e.g. while spamming massive numbers of empty blocks) from causing latency spikes elsewhere in the executor
static NS_EXECUTOR: Lazy<&'static smol::Executor<'static>> = Lazy::new(|| {
    let exec = Box::leak(Box::new(smol::Executor::new()));
    let exec: &'static smol::Executor<'static> = exec;
    log::warn!("starting novasymph executor");
    // spin off one thread
    std::thread::Builder::new()
        .name("novasymph".into())
        .spawn(move || smol::future::block_on(exec.run(smol::future::pending::<()>())))
        .unwrap();
    exec
});
