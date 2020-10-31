use std::collections::BTreeMap;

use bytes::Bytes;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Request {
    pub verb: String,
    pub args: BTreeMap<String, Bytes>
}

impl Request {
    /// Creates a new request with the given verb
    pub fn verb(verb: &str) -> Self {
        Self{verb: verb.into(), args: BTreeMap::new()}
    }
    /// Adds an argument to the request
    pub fn arg(mut self, param: &str, value: impl Serialize) -> Self {
        self.args.insert(param.into(), bincode::serialize(&value).unwrap().into());
        self
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum Response {
    Okay(Bytes),
    Error(String)
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RoutingRequest {
    pub proto: String,
    pub addr: String,
}
