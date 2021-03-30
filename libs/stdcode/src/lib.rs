use bincode::Options;
use serde::{de::DeserializeOwned, Serialize};
pub mod hex;
pub mod hex32;

/// Safe deserialize that prevents DoS attacks.
pub fn deserialize<T: DeserializeOwned>(bts: &[u8]) -> bincode::Result<T> {
    bincode::DefaultOptions::new()
        .with_varint_encoding()
        .reject_trailing_bytes()
        .with_limit(bts.len() as u64)
        .deserialize(bts)
}

/// Serialize the stuff
pub fn serialize<T: Serialize>(v: &T) -> bincode::Result<Vec<u8>> {
    bincode::DefaultOptions::new()
        .with_varint_encoding()
        .reject_trailing_bytes()
        .serialize(v)
}
