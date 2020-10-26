use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RawRequest {
    pub proto_ver: u8,
    pub netname: String,
    pub verb: String,
    pub payload: Vec<u8>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RawResponse {
    pub kind: String,
    pub body: Vec<u8>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RoutingRequest {
    pub proto: String,
    pub addr: String,
}
