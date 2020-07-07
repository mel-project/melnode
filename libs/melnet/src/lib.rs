use rlp::{Decodable, Encodable};
use thiserror::Error;
mod connpool;
use connpool::*;
mod routingtable;
use derivative::*;
use log::debug;
use routingtable::*;
use std::convert::TryInto;
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::Arc;
mod reqs;
use futures::prelude::*;
use im::HashMap;
use reqs::*;
use smol::*;

const PROTO_VER: u8 = 1;
const MAX_MSG_SIZE: u32 = 10 * 1024 * 1024;

type Result<T> = std::result::Result<T, MelnetError>;

type VerbHandler = Arc<dyn Fn(&NetState, &[u8]) -> Result<Vec<u8>> + Send + Sync>;

#[derive(Error, Debug)]
pub enum MelnetError {
    #[error("custom error: `{0}`")]
    Custom(String),
    #[error("verb not found")]
    VerbNotFound,
    #[error("network error: `{0}`")]
    Network(std::io::Error),
}

#[derive(Derivative, Default, Clone)]
#[derivative(Debug)]
/// A clonable structure representing a melnet state. All copies share the same routing table.
pub struct NetState {
    conn_pool: Arc<ConnPool>,
    routes: Arc<RoutingTable>,
    #[derivative(Debug = "ignore")]
    verbs: HashMap<String, VerbHandler>,
}

impl NetState {
    /// Consumes the netstate and runs it, blocking until the listener no longer gives things. You should clone a copy first in order to use the netstate as a client.
    pub async fn run_server(mut self, listener: Async<TcpListener>) {
        self.setup_routing();
        loop {
            let conn = listener.accept().await;
            let self_copy = self.clone();
            match conn {
                Ok((conn, _)) => {
                    //let conn: Async<TcpStream> = conn;
                    Task::spawn(async move {
                        let addr = conn.get_ref().peer_addr().unwrap();
                        if let Err(err) = self_copy.server_handle(conn).await {
                            debug!("conn from {:?} error {:?}", addr, err);
                        }
                    })
                    .detach();
                }
                Err(err) => {
                    debug!("exiting listener due to {:?}", err);
                    return;
                }
            }
        }
    }

    async fn server_handle(
        &self,
        mut conn: Async<TcpStream>,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        conn.get_ref()
            .set_write_timeout(Some(std::time::Duration::from_secs(10)))?;
        conn.get_ref()
            .set_read_timeout(Some(std::time::Duration::from_secs(60)))?;
        loop {
            // read command
            let cmd: RawRequest = rlp::decode(&read_len_bts(&mut conn).await?)?;
            if cmd.proto_ver != 1 {
                let err = rlp::encode(&RawResponse {
                    kind: "Err".to_owned(),
                    body: rlp::encode(&"bad protocol version"),
                });
                write_len_bts(&mut conn, &err).await?;
                continue;
            }
            // respond to command
            let responder = self.verbs.get(&cmd.verb);
            match responder {
                None => {
                    write_len_bts(
                        &mut conn,
                        &rlp::encode(&RawResponse {
                            kind: "NoVerb".to_owned(),
                            body: b"".to_vec(),
                        }),
                    )
                    .await?;
                    continue;
                }
                Some(responder) => {
                    let ss = self.clone();
                    let responder = responder.clone();
                    let response = blocking!(responder(&ss, &cmd.payload));
                    match response {
                        Ok(response) => {
                            write_len_bts(
                                &mut conn,
                                &rlp::encode(&RawResponse {
                                    kind: "Ok".to_owned(),
                                    body: rlp::encode(&response),
                                }),
                            )
                            .await?;
                        }
                        Err(MelnetError::Custom(err)) => {
                            write_len_bts(
                                &mut conn,
                                &rlp::encode(&RawResponse {
                                    kind: "Err".to_owned(),
                                    body: err.as_bytes().to_owned(),
                                }),
                            )
                            .await?
                        }
                        _ => break,
                    }
                }
            }
        }
        Ok(())
    }

    /// Registers the handler for new_peer.
    fn setup_routing(&mut self) {
        // ping just responds to a u64 with itself
        self.register_verb("ping", |_, ping: u64| Ok(ping));
        // new_addr
        self.register_verb("new_addr", |state, rr: RoutingRequest| {
            debug!("got new_addr {:?}", rr);
            // first we check the aliveness

            Ok("")
        })
    }

    /// Registers a verb that takes in an RLP object and returns an RLP object.
    pub fn register_verb<TInput: Decodable, TOutput: Encodable>(
        &mut self,
        verb: &str,
        cback: impl Fn(&NetState, TInput) -> Result<TOutput> + 'static + Send + Sync,
    ) {
        self.verbs.insert(verb.to_owned(), erase_cback_types(cback));
    }
    /// Returns a function that does a melnet request to any given endpoint.
    pub async fn request<TInput: Encodable, TOutput: Decodable>(
        &self,
        addr: impl ToSocketAddrs,
        verb: &str,
        req: TInput,
    ) -> Result<TOutput> {
        // grab a connection
        let mut conn = self
            .conn_pool
            .connect(addr)
            .await
            .map_err(MelnetError::Network)?;
        // send a request
        let rr = rlp::encode(&RawRequest {
            proto_ver: PROTO_VER,
            verb: verb.to_owned(),
            payload: rlp::encode(&req),
        });
        conn.get_ref()
            .set_write_timeout(Some(std::time::Duration::from_secs(10)))
            .map_err(MelnetError::Network)?;
        conn.get_ref()
            .set_read_timeout(Some(std::time::Duration::from_secs(60)))
            .map_err(MelnetError::Network)?;
        write_len_bts(&mut conn, &rr).await?;
        // read the response length
        let response: RawResponse = rlp::decode(&read_len_bts(&mut conn).await?).map_err(|e| {
            MelnetError::Network(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })?;
        let response = match response.kind.as_ref() {
            "Ok" => rlp::decode::<TOutput>(&response.body)
                .map_err(|_| MelnetError::Custom("rlp error".to_owned()))?,
            "NoVerb" => return Err(MelnetError::VerbNotFound),
            _ => {
                return Err(MelnetError::Custom(
                    String::from_utf8_lossy(&response.body).to_string(),
                ))
            }
        };
        // put the connection back
        self.conn_pool.recycle(conn);
        Ok(response)
    }
}

async fn read_len_bts<T: AsyncRead + Unpin>(conn: &mut T) -> Result<Vec<u8>> {
    // read the response length
    let mut response_len = [0; 4];
    conn.read_exact(&mut response_len)
        .await
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
        .await
        .map_err(MelnetError::Network)?;
    Ok(response_buf)
}

async fn write_len_bts<T: AsyncWrite + Unpin>(conn: &mut T, rr: &[u8]) -> Result<()> {
    conn.write_all(&(rr.len() as u32).to_be_bytes())
        .await
        .map_err(MelnetError::Network)?;
    conn.write_all(&rr).await.map_err(MelnetError::Network)?;
    conn.flush().await.map_err(MelnetError::Network)?;
    Ok(())
}

fn erase_cback_types<TInput: Decodable, TOutput: Encodable>(
    cback: impl Fn(&NetState, TInput) -> Result<TOutput> + 'static + Send + Sync,
) -> VerbHandler {
    Arc::new(move |state, input| {
        let input: TInput =
            rlp::decode(input).map_err(|e| MelnetError::Custom(format!("rlp error: {:?}", e)))?;
        let output: TOutput = cback(state, input)?;
        Ok(rlp::encode(&output))
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
