use rlp_derive::*;
#[derive(RlpEncodable, RlpDecodable, Clone)]
pub struct Script(pub Vec<u8>);
