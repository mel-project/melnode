Loki Logger
===
[![Build Status](https://github.com/nwmqpa/loki-logger/workflows/build/badge.svg)](https://github.com/nwmqpa/loki-logger/actions)
[![Crates.io](https://img.shields.io/crates/v/loki-logger.svg)](https://crates.io/crates/loki-logger)
[![Crates.io](https://img.shields.io/crates/l/loki-logger.svg)](https://crates.io/crates/loki-logger)
[![Documentation](https://img.shields.io/badge/documentation-docs.rs-blue.svg)](https://docs.rs/loki-logger)

A [loki](https://grafana.com/oss/loki/) logger for the [`log`](https://crates.io/crates/log) facade.

## Examples

```rust
extern crate log;
extern crate loki_logger;
use log::LevelFilter;

#[tokio::main]
async fn main() {
    loki_logger::init(
        "http://loki:3100/loki/api/v1/push",
        log::LevelFilter::Info,
    ).unwrap();

    log::info!("Logged into Loki !");
}
```


```rust
extern crate log;
extern crate loki_logger;
use std::iter::FromIterator;
use std::collections::HashMap;
use log::LevelFilter;

#[tokio::main]
async fn main() {
    let initial_labels = HashMap::from_iter([
        ("application".to_string(), "loki_logger".to_string()),
        ("environment".to_string(), "development".to_string()),
    ]);

    loki_logger::init_with_labels(
        "http://loki:3100/loki/api/v1/push",
        log::LevelFilter::Info,
        initial_labels,
    ).unwrap();

    log::info!("Logged into Loki !");
}
```

### Use with extra labels

Starting from 0.4.7, the [`log`](https://crates.io/crates/log) crate started introducing the new key/value system for structured logging.

This crate makes heavy use of such system as to create and send custom loki labels.

If you want to use this system, you have to use the git version of the log crate and enable the [`kv_unstable`](https://docs.rs/crate/log/0.4.14/features#kv_unstable) feature:

```toml
[dependencies.log]
# It is recommended that you pin this version to a specific commit to avoid issues.
git = "https://github.com/rust-lang/log.git"
features = ["kv_unstable"]
```

This feature will allow you to use the [`log`](https://crates.io/crates/log) facade as such:

```rust
extern crate log;
extern crate loki_logger;
use std::iter::FromIterator;
use std::collections::HashMap;
use log::LevelFilter;

#[tokio::main]
async fn main() {
    let initial_labels = HashMap::from_iter([
        ("application".to_string(), "loki_logger".to_string()),
        ("environment".to_string(), "development".to_string()),
    ]);

    loki_logger::init_with_labels(
        "http://loki:3100/loki/api/v1/push",
        log::LevelFilter::Info,
        initial_labels,
    ).unwrap();

    // Due to stabilization issue, this is still unstable,
    // the log macros needs to have at least one formatting parameter for this to work.
    log::info!(foo = "bar"; "Logged into Loki !{}", "");
}
```
