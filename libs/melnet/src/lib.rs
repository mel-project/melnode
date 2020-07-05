use rlp::{Decodable, Encodable};
use std::collections::HashMap;
use thiserror::Error;
mod connpool;
use connpool::*;
mod routingtable;
use derivative::*;
use parking_lot::Mutex;
use rlp_derive::{RlpDecodable, RlpEncodable};
use routingtable::*;
use std::io::prelude::*;
use std::net::ToSocketAddrs;
mod reqs;
use reqs::*;

const PROTO_VER: u8 = 1;
const MAX_MSG_SIZE: u32 = 10 * 1024 * 1024;

type Result<T> = std::result::Result<T, MelnetError>;

type VerbHandler<'a> = Box<dyn FnMut(&'a NetState, &[u8]) -> Result<Vec<u8>> + 'a>;

#[derive(Error, Debug)]
pub enum MelnetError {
    #[error("custom error: `{0}`")]
    Custom(String),
    #[error("verb not found")]
    VerbNotFound,
    #[error("network error: `{0}`")]
    Network(std::io::Error),
}

#[derive(Derivative, Default)]
#[derivative(Debug)]
pub struct NetState<'a> {
    conn_pool: ConnPool,
    routes: RoutingTable,
    #[derivative(Debug = "ignore")]
    verbs: HashMap<String, VerbHandler<'a>>,
}

impl<'a> NetState<'a> {
    // Registers a verb that takes in an RLP object and returns an RLP object.
    pub fn register_verb<TInput: Decodable, TOutput: Encodable>(
        &mut self,
        verb: &str,
        cback: &'a mut dyn FnMut(&NetState, TInput) -> Result<TOutput>,
    ) {
        self.verbs.insert(
            verb.to_owned(),
            Box::new(move |state, in_as_bts| {
                let input: TInput = rlp::decode(in_as_bts)
                    .map_err(|e| MelnetError::Custom(format!("rlp error: {:?}", e)))?;
                Ok(rlp::encode(&cback(state, input)?))
            }),
        );
    }
    // Does a melnet request to any given endpoint.
    pub fn request<TInput: Encodable, TOutput: Decodable>(
        &self,
        addr: impl ToSocketAddrs,
        verb: &str,
        req: TInput,
    ) -> Result<TOutput> {
        // grab a connection
        let mut conn = self.conn_pool.connect(addr).map_err(MelnetError::Network)?;
        // send a request
        let rr = rlp::encode(&RawRequest {
            proto_ver: PROTO_VER,
            verb: verb.to_owned(),
            payload: rlp::encode(&req),
        });
        conn.set_write_timeout(Some(std::time::Duration::from_secs(10)))
            .map_err(MelnetError::Network)?;
        conn.set_read_timeout(Some(std::time::Duration::from_secs(60)))
            .map_err(MelnetError::Network)?;
        conn.write_all(&(rr.len() as u32).to_be_bytes())
            .map_err(MelnetError::Network)?;
        conn.write_all(&rr).map_err(MelnetError::Network)?;
        conn.flush().map_err(MelnetError::Network)?;
        // read the response length
        let mut response_len = [0; 4];
        conn.read_exact(&mut response_len)
            .map_err(MelnetError::Network)?;
        let response_len = u32::from_be_bytes(response_len);
        if response_len > MAX_MSG_SIZE {
            return Err(MelnetError::Network(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "response too big",
            )));
        }
        // read the response
        let mut response_buf = vec![0; response_len as usize];
        conn.read_exact(&mut response_buf)
            .map_err(MelnetError::Network)?;
        let response: TOutput = rlp::decode(&response_buf).map_err(|e| {
            MelnetError::Network(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })?;
        // put the connection back
        self.conn_pool.recycle(conn);
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
