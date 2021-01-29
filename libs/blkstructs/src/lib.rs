//! This crate contains the data structures and core algorithms that comprise Themelio's core state machine.
//! Any piece of software needing to parse Themelio data, validate Themelio transactions, or answer questions like
//! "what happens to the Themelio state if transactions A, B, and C happen" can use this minimal-depedency crate.
//!
//! Roughly, the structs in this crate are organized as follows:
//! - `State` represents a full Themelio world-state and it's not directly serializable. It includes *all* the information needed to validate new transactions and blocks, such as a SMT of all outstanding coins, Melmint parameters, etc. It has methods taking `Transaction`s etc that advance the state, as well as others to produce serializable blocks, headers, etc.
//! - `Transaction` represents a serializable Themelio transaction. It has some helper methods to count coins, estimate fees, etc, largely to help build wallets.
//! - `StakeDoc`, which every `State` includes, encapsulates the Symphonia epoch-based stake information.
//! - `SmtMapping` represents a type-safe SMT-backed mapping that is extensively used within the crate.

// #![feature(test)]

mod constants;
pub mod melscript;
mod stake;
mod state;
mod transaction;
use bincode::Options;
pub use constants::*;
mod smtmapping;
use serde::de::DeserializeOwned;
pub use smtmapping::*;
pub use state::*;
pub use transaction::*;

/// Safe deserialize that prevents DoS attacks.
pub fn safe_deserialize<T: DeserializeOwned>(bts: &[u8]) -> bincode::Result<T> {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .reject_trailing_bytes()
        .with_limit(bts.len() as u64)
        .deserialize(bts)
}

mod testing;
