use std::convert::TryInto;

pub use anyhow::Result;
pub use parking_lot::RwLock;
pub use smol::prelude::*;

use serde::{Deserialize, Serialize};
use smol::net::TcpListener;
pub use smol::{Task, Timer};
// /// Guesses the public IP address of the current machine.
// async fn guess_my_ip() -> Result<String> {
//     // TODO: something better-quality
//     let response = smol::unblock(move || {
//         attohttpc::get(
//             "http://checkip.amazonaws.com/
//     ",
//         )
//         .send()
//     })
//     .await?;
//     Ok(response.text()?.trim().to_owned())
// }

/// Creates a new melnet state with a default route.
pub async fn new_melnet(listener: &TcpListener, name: &str) -> Result<melnet::NetState> {
    // let my_ip = guess_my_ip().await?;
    // let my_ip_port = format!("{}:{}", my_ip, listener.local_addr()?.port());
    let net = melnet::NetState::new_with_name(name);
    // net.add_route(my_ip_port.to_socket_addrs()?.next().unwrap());
    Ok(net)
}

use blkstructs::Transaction;
use tmelcrypt::HashVal;
/// Request for a new block.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewBlkRequest {
    pub header: blkstructs::Header,
    pub txhashes: Vec<HashVal>,
    pub partial_transactions: Vec<Transaction>,
}

/// Response for a new block request.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewBlkResponse {
    pub missing_txhashes: Vec<HashVal>,
}
