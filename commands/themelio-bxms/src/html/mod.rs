mod homepage;
use std::fmt::Display;

use blkstructs::MICRO_CONVERTER;
mod block;
pub use block::*;
pub use homepage::*;

// A wrapper for microunit-denominated values
struct MicroUnit(u128, String);

impl Display for MicroUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{} {}",
            self.0 / MICRO_CONVERTER,
            self.0 % MICRO_CONVERTER,
            self.1
        )
    }
}
