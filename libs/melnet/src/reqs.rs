use rlp_derive::{RlpDecodable, RlpEncodable};

#[derive(RlpEncodable, RlpDecodable, Debug, Clone)]
pub struct RawRequest {
    pub proto_ver: u8,
    pub verb: String,
    pub payload: Vec<u8>,
}

#[derive(RlpEncodable, RlpDecodable, Debug, Clone)]
pub struct RawResponse {
    pub kind: String,
    pub body: Vec<u8>,
}

#[derive(RlpEncodable, RlpDecodable, Debug, Clone)]
pub struct RoutingRequest {
    pub proto: String,
    pub addr: String,
}
