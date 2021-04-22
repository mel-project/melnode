mod homepage;
use std::{fmt::Display, time::Instant};

use blkstructs::MICRO_CONVERTER;
mod block;
mod transaction;
pub use block::*;
pub use homepage::*;
pub use transaction::*;

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

// A wrapper for calculating rendering times
struct RenderTimeTracer<'a> {
    start_time: Instant,
    label: &'a str,
}

impl<'a> Drop for RenderTimeTracer<'a> {
    fn drop(&mut self) {
        log::debug!(
            "rendering {} took {:?}",
            self.label,
            self.start_time.elapsed()
        );
    }
}

impl<'a> RenderTimeTracer<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            start_time: Instant::now(),
            label,
        }
    }
}
