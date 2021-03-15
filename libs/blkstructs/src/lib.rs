//! This crate contains the data structures and core algorithms that comprise Themelio's core state machine.
//! Any piece of software needing to parse Themelio data, validate Themelio transactions, or answer questions like
//! "what happens to the Themelio state if transactions A, B, and C happen" can use this minimal-depedency crate.
//!
//! Roughly, the structs in this crate are organized as follows:
//! - `State` represents a full Themelio world-state and it's not directly serializable. It includes *all* the information needed to validate new transactions and blocks, such as a SMT of all outstanding coins, Melmint parameters, etc. It has methods taking `Transaction`s etc that advance the state, as well as others to produce serializable blocks, headers, etc.
//! - `Transaction` represents a serializable Themelio transaction. It has some helper methods to count coins, estimate fees, etc, largely to help build wallets.
//! - `StakeDoc`, which every `State` includes, encapsulates the Symphonia epoch-based stake information.
//! - `SmtMapping` represents a type-safe SMT-backed mapping that is extensively used within the crate.
mod constants;
pub mod melvm;
mod stake;
mod state;
pub use state::melmint::*;
mod transaction;
pub use constants::*;
mod smtmapping;
pub use fastsync::*;
pub use smtmapping::*;
pub use state::melswap::PoolState;
pub use state::*;
pub use transaction::*;

mod fastsync;

#[cfg(test)]
#[macro_use]
extern crate lazy_static;

mod testing;
