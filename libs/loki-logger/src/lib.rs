#![deny(missing_docs)]
#![deny(rustdoc::missing_doc_code_examples)]
//! A [loki](https://grafana.com/oss/loki/) logger for the [`log`](https://crates.io/crates/log) facade.
//! One event is written and send to loki per log call. Each event contain the time in nano second
//! it was scheduled to be sent, in most cases, when the logging occured.
//!
//! # Examples
//!
//! You simply need to specify your [loki push URL](https://grafana.com/docs/loki/latest/api/#post-lokiapiv1push) and the minimum log level to start the logger.
//!
//! ```rust
//! # extern crate log;
//! # extern crate loki_logger;
//! use log::LevelFilter;
//!
//! # #[tokio::main]
//! # async fn main() {
//! loki_logger::init(
//!     "http://loki:3100/loki/api/v1/push",
//!     log::LevelFilter::Info,
//! ).unwrap();
//!
//! log::info!("Logged into Loki !");
//! # }
//! ```
//!
//! Or specify [static labels](https://grafana.com/docs/loki/latest/best-practices/#static-labels-are-good) to use in your loki streams.
//! Those labels are overwriten by event-specific label, if any.
//!
//! ```rust
//! # extern crate log;
//! # extern crate loki_logger;
//! # use std::iter::FromIterator;
//! use std::collections::HashMap;
//! use log::LevelFilter;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let initial_labels = HashMap::from_iter([
//!     ("application".to_string(), "loki_logger".to_string()),
//!     ("environment".to_string(), "development".to_string()),
//! ]);
//!
//! loki_logger::init_with_labels(
//!     "http://loki:3100/loki/api/v1/push",
//!     log::LevelFilter::Info,
//!     initial_labels,
//! ).unwrap();
//!
//! log::info!("Logged into Loki !");
//! # }
//! ```
//! # Log format
//!
//! Each and every log event sent to loki will contain at least the [`level`](log::Level) of the event as well as the [time in nanoseconds](std::time::Duration::as_nanos) of the event scheduling.
//!
//! # Notice on extra labels
//!
//! Starting from 0.4.7, the [`log`](https://crates.io/crates/log) crate started introducing the new key/value system for structured logging.
//!
//! The loki_logger crate makes heavy use of such system as to create and send custom loki labels.
//!
//! If you want to use the key:value tag system, you have to use the git version of the log crate and enable the [`kv_unstable`](https://docs.rs/crate/log/0.4.14/features#kv_unstable) feature:
//!
//! ```toml
//! [dependencies.log]
//! # It is recommended that you pin this version to a specific commit to avoid issues.
//! git = "https://github.com/rust-lang/log.git"
//! branch = "kv_macro"
//! features = ["kv_unstable"]
//! ```
//! The ability to use the key:value system with the log crate's macros should come up with the 0.4.15 release or afterwards.
//!
//! The kv_unstable feature allows you to use the [`log`](https://crates.io/crates/log) facade as such:
//!
//! ```ignore
//! # extern crate log;
//! # extern crate loki_logger;
//! # use std::iter::FromIterator;
//! use std::collections::HashMap;
//! use log::LevelFilter;
//!
//! # #[tokio::main]
//! # async fn main() {
//!
//! loki_logger::init(
//!     "http://loki:3100/loki/api/v1/push",
//!     log::LevelFilter::Info,
//! ).unwrap();
//!
//! // Due to stabilization issue, this is still unstable,
//! // the log macros needs to have at least one formatting parameter for this to work.
//! log::info!(foo = "bar"; "Logged into Loki !{}", "");
//! # }
//! ```
//!
//! # Notice on asynchronous execution
//!
//! The loki_logger crate ships with asynchronous execution, orchestrated with [`tokio`](https://tokio.rs/), by default.
//!
//! This means that for the logging operations to work, you need to be in the scope of a asynchronous runtime first.
//!
//! Otherwise, you can activate the `blocking` feature of The loki_logger crate to use a blocking client.
//!
//! THIS IS NOT RECOMMENDED FOR PRODUCTIONS WORKLOAD.

use serde::Serialize;
use std::{
    collections::HashMap,
    error::Error,
    time::{SystemTime, UNIX_EPOCH},
};

use log::{
    kv::{Source, Visitor},
    LevelFilter, Metadata, Record, SetLoggerError,
};

/// Re-export of the log crate for use with a different version by the `loki-logger` crate's user.
pub use log;

#[derive(Serialize)]
struct LokiStream {
    stream: HashMap<String, String>,
    values: Vec<[String; 2]>,
}

#[derive(Serialize)]
struct LokiRequest {
    streams: Vec<LokiStream>,
}

#[cfg(not(feature = "blocking"))]
struct LokiLogger {
    url: String,
    initial_labels: Option<HashMap<String, String>>,
    client: reqwest::Client,
}

#[cfg(feature = "blocking")]
struct LokiLogger {
    url: String,
    initial_labels: Option<HashMap<String, String>>,
    client: reqwest::blocking::Client,
}

fn init_inner<S: AsRef<str>>(
    url: S,
    max_log_level: LevelFilter,
    initial_labels: Option<HashMap<String, String>>,
) -> Result<(), SetLoggerError> {
    let logger = Box::new(LokiLogger::new(url, initial_labels));
    log::set_boxed_logger(logger).map(|()| log::set_max_level(max_log_level))
}

/// Configure the [`log`](https://crates.io/crates/log) facade to log to [loki](https://grafana.com/oss/loki/).
///
/// This function initialize the logger with no defaults [static labels](https://grafana.com/docs/loki/latest/best-practices/#static-labels-are-good).
/// To use them, you may want to use [`init_with_labels`].
///
/// # Example
///
/// Usage:
///
/// ```rust
/// # extern crate log;
/// # extern crate loki_logger;
/// use log::LevelFilter;
///
/// # #[tokio::main]
/// # async fn main() {
/// loki_logger::init(
///     "http://loki:3100/loki/api/v1/push",
///     log::LevelFilter::Info,
/// ).unwrap();
///
/// log::info!("Logged into Loki !");
/// # }
/// ```
pub fn init<S: AsRef<str>>(url: S, max_log_level: LevelFilter) -> Result<(), SetLoggerError> {
    init_inner(url, max_log_level, None)
}

/// Configure the [`log`](https://crates.io/crates/log) facade to log to [loki](https://grafana.com/oss/loki/).
///
/// This function initialize the logger with defaults [static labels](https://grafana.com/docs/loki/latest/best-practices/#static-labels-are-good).
/// To not use them, you may want to use [`init`].
///
/// # Example
///
/// Usage:
///
/// ```rust
/// # extern crate log;
/// # extern crate loki_logger;
/// # use std::iter::FromIterator;
/// use std::collections::HashMap;
/// use log::LevelFilter;
///
/// # #[tokio::main]
/// # async fn main() {
/// let initial_labels = HashMap::from_iter([
///     ("application".to_string(), "loki_logger".to_string()),
///     ("environment".to_string(), "development".to_string()),
/// ]);
///
/// loki_logger::init_with_labels(
///     "http://loki:3100/loki/api/v1/push",
///     log::LevelFilter::Info,
///     initial_labels
/// ).unwrap();
///
/// log::info!("Logged into Loki !");
/// # }
/// ```
pub fn init_with_labels<S: AsRef<str>>(
    url: S,
    max_log_level: LevelFilter,
    initial_labels: HashMap<String, String>,
) -> Result<(), SetLoggerError> {
    init_inner(url, max_log_level, Some(initial_labels))
}

struct LokiVisitor<'kvs> {
    values: HashMap<log::kv::Key<'kvs>, log::kv::Value<'kvs>>,
}

impl<'kvs> LokiVisitor<'kvs> {
    pub fn new(count: usize) -> Self {
        Self {
            values: HashMap::with_capacity(count),
        }
    }

    pub fn read_kv(
        &'kvs mut self,
        source: &'kvs dyn Source,
    ) -> Result<&HashMap<log::kv::Key<'kvs>, log::kv::Value<'kvs>>, log::kv::Error> {
        for _ in 0..source.count() {
            source.visit(self)?;
        }
        Ok(&self.values)
    }
}

impl<'kvs> Visitor<'kvs> for LokiVisitor<'kvs> {
    fn visit_pair(
        &mut self,
        key: log::kv::Key<'kvs>,
        value: log::kv::Value<'kvs>,
    ) -> Result<(), log::kv::Error> {
        self.values.insert(key, value);
        Ok(())
    }
}

impl log::Log for LokiLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            if let Err(e) = self.log_event_record(record) {
                eprintln!("Impossible to log event to loki: {:?}", e)
            }
        }
    }

    fn flush(&self) {}
}

impl LokiLogger {
    #[cfg(not(feature = "blocking"))]
    fn new<S: AsRef<str>>(url: S, initial_labels: Option<HashMap<String, String>>) -> Self {
        Self {
            url: url.as_ref().to_string(),
            initial_labels,
            client: reqwest::Client::new(),
        }
    }

    #[cfg(feature = "blocking")]
    fn new<S: AsRef<str>>(url: S, initial_labels: Option<HashMap<String, String>>) -> Self {
        Self {
            url: url.as_ref().to_string(),
            initial_labels,
            client: reqwest::blocking::Client::new(),
        }
    }

    #[cfg(not(feature = "blocking"))]
    fn log_to_loki(
        &self,
        message: String,
        labels: HashMap<String, String>,
    ) -> Result<(), Box<dyn Error>> {
        let client = self.client.clone();
        let url = self.url.clone();

        let loki_request = make_request(message, labels)?;
        tokio::spawn(async move {
            if let Err(e) = client.post(url).json(&loki_request).send().await {
                eprintln!("{:?}", e);
            };
        });
        Ok(())
    }

    #[cfg(feature = "blocking")]
    fn log_to_loki(
        &self,
        message: String,
        labels: HashMap<String, String>,
    ) -> Result<(), Box<dyn Error>> {
        let url = self.url.clone();

        let loki_request = make_request(message, labels)?;
        self.client.post(url).json(&loki_request).send()?;
        Ok(())
    }

    fn merge_loki_labels(
        &self,
        kv_labels: &HashMap<log::kv::Key, log::kv::Value>,
    ) -> HashMap<String, String> {
        merge_labels(self.initial_labels.as_ref(), kv_labels)
    }

    fn log_event_record(&self, record: &Record) -> Result<(), Box<dyn Error>> {
        let kv = record.key_values();
        let mut visitor = LokiVisitor::new(kv.count());
        let values = visitor.read_kv(kv)?;
        let message = format!("{:?}", record.args());
        let mut labels = self.merge_loki_labels(values);
        labels.insert(
            "level".to_string(),
            record.level().to_string().to_ascii_lowercase(),
        );
        self.log_to_loki(message, labels)
    }
}

fn merge_labels(
    initial_labels: Option<&HashMap<String, String>>,
    kv_labels: &HashMap<log::kv::Key, log::kv::Value>,
) -> HashMap<String, String> {
    let mut labels = if let Some(initial_labels) = initial_labels {
        initial_labels.clone()
    } else {
        HashMap::with_capacity(kv_labels.len())
    };
    labels.extend(
        kv_labels
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string())),
    );
    labels
}

fn make_request(
    message: String,
    labels: HashMap<String, String>,
) -> Result<LokiRequest, Box<dyn Error>> {
    let start = SystemTime::now();
    let time_ns = time_offset_since(start)?;
    let loki_request = LokiRequest {
        streams: vec![LokiStream {
            stream: labels,
            values: vec![[time_ns, message]],
        }],
    };
    Ok(loki_request)
}

fn time_offset_since(start: SystemTime) -> Result<String, Box<dyn Error>> {
    let since_start = start.duration_since(UNIX_EPOCH)?;
    let time_ns = since_start.as_nanos().to_string();
    Ok(time_ns)
}

#[cfg(test)]
mod tests {
    use log::kv::{Key, Value};

    use crate::{merge_labels, time_offset_since};
    use std::{
        collections::HashMap,
        time::{Duration, SystemTime},
    };

    #[test]
    fn time_offsets() {
        let t1 = time_offset_since(SystemTime::now());
        assert!(t1.is_ok());

        // Constructing a negative timestamp
        let negative_time = SystemTime::UNIX_EPOCH.checked_sub(Duration::from_secs(1));

        assert!(negative_time.is_some());

        let t2 = time_offset_since(negative_time.unwrap());
        assert!(t2.is_err());
    }

    #[test]
    fn merge_no_initial_labels() {
        let kv_labels = HashMap::new();
        let merged_labels = merge_labels(None, &kv_labels);

        assert_eq!(merged_labels, HashMap::new());

        let kv_labels = [
            (Key::from_str("application"), Value::from("loki_logger")),
            (Key::from_str("environment"), Value::from("development")),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();
        let merged_labels = merge_labels(None, &kv_labels);

        assert_eq!(
            merged_labels,
            [
                ("application".to_string(), "loki_logger".to_string()),
                ("environment".to_string(), "development".to_string())
            ]
            .into_iter()
            .collect::<HashMap<_, _>>()
        );
    }

    #[test]
    fn merge_initial_labels() {
        let kv_labels = HashMap::new();
        let initial_labels = HashMap::new();
        let merged_labels = merge_labels(Some(&initial_labels), &kv_labels);

        assert_eq!(merged_labels, HashMap::new());

        let kv_labels = HashMap::new();
        let initial_labels = [
            ("application".to_string(), "loki_logger".to_string()),
            ("environment".to_string(), "development".to_string()),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();
        let merged_labels = merge_labels(Some(&initial_labels), &kv_labels);

        assert_eq!(merged_labels, initial_labels);

        let initial_labels = [
            ("application".to_string(), "loki_logger".to_string()),
            ("environment".to_string(), "development".to_string()),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();
        let kv_labels = [
            (Key::from_str("event_name"), Value::from("request")),
            (Key::from_str("handler"), Value::from("/loki/api/v1/push")),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();
        let merged_labels = merge_labels(Some(&initial_labels), &kv_labels);

        assert_eq!(
            merged_labels,
            [
                ("application".to_string(), "loki_logger".to_string()),
                ("environment".to_string(), "development".to_string()),
                ("event_name".to_string(), "request".to_string()),
                ("handler".to_string(), "/loki/api/v1/push".to_string())
            ]
            .into_iter()
            .collect::<HashMap<_, _>>()
        );
    }

    #[test]
    fn merge_overwrite_labels() {
        let initial_labels = [
            ("application".to_string(), "loki_logger".to_string()),
            ("environment".to_string(), "development".to_string()),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();
        let kv_labels = [
            (Key::from_str("event_name"), Value::from("request")),
            (Key::from_str("environment"), Value::from("production")),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();
        let merged_labels = merge_labels(Some(&initial_labels), &kv_labels);

        assert_eq!(
            merged_labels,
            [
                ("application".to_string(), "loki_logger".to_string()),
                ("environment".to_string(), "production".to_string()),
                ("event_name".to_string(), "request".to_string()),
            ]
            .into_iter()
            .collect::<HashMap<_, _>>()
        );
    }
}
