pub use anyhow::Result;
pub use futures::prelude::*;
pub use parking_lot::RwLock;

use serde::{Deserialize, Serialize};
use smol::net::TcpListener;
pub use smol::{Task, Timer};
use std::convert::TryInto;
use std::net::ToSocketAddrs;
pub const TEST_ANET: &str = "themelio-test-alphanet";
//use std::pin::Pin;

//pub type PinBoxFut<T> = Pin<Box<dyn Future<Output = T> + 'static>>;

/// Guesses the public IP address of the current machine.
async fn guess_my_ip() -> Result<String> {
    // TODO: something better-quality
    let response = smol::unblock(move || {
        attohttpc::get(
            "http://checkip.amazonaws.com/
    ",
        )
        .send()
    })
    .await?;
    Ok(response.text()?.trim().to_owned())
}

/// Creates a new melnet state with a default route.
pub async fn new_melnet(listener: &TcpListener, name: &str) -> Result<melnet::NetState> {
    let my_ip = guess_my_ip().await?;
    let my_ip_port = format!("{}:{}", my_ip, listener.local_addr()?.port());
    let net = melnet::NetState::new_with_name(name);
    net.add_route(my_ip_port.to_socket_addrs()?.next().unwrap());
    Ok(net)
}

use blkstructs::Transaction;
use tmelcrypt::HashVal;
/// Request for a new block.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewBlkRequest {
    pub consensus: symphonia::QuorumCert,
    pub header: blkstructs::Header,
    pub txhashes: Vec<HashVal>,
    pub partial_transactions: Vec<Transaction>,
}

/// Response for a new block request.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewBlkResponse {
    pub missing_txhashes: Vec<HashVal>,
}

/// Generates a phony public and secret key for the testnet. This returns a hardcoded key pair given 0<=i<10.
pub fn insecure_testnet_keygen(i: usize) -> (tmelcrypt::Ed25519PK, tmelcrypt::Ed25519SK) {
    let pkk = [
        "fd296a6fb3b0840a0371e950771cfcb0ab78dd47ffd7165046658fe0e6ebff69",
        "7c5ceef673eed1c788c0c42443fef0d8768b2970f4850358ec8b77911cd2bca1",
        "b8c3c3c1e3ce31ad3396cc1b95663125732113a97dda450c14bdcfff42101894",
        "50eea014502f98e678902fea5e385f86c295be9df9685a0c328a5a4cd6257cfb",
        // "ff8994c927dfc7be0f155b0113f5fa1e2b93059387b22d743fa722c9e8fa1236",
        // "5f827e2ece2b4148e4d61da3f9c049a87c5c61331b9d82617115a83f5a4cc735",
        // "be9a5490aaf7ec26e9ff509fbe488f35d89355a659f72c041cf1e2e797ba0aa8",
        // "d8c868ee748202290c38f1a176676325423a4491f45d1010fe3ef02ac610727d",
        // "7cb1442b2da924d8e1e16d33a853b3bfc16b42b83b7ef1c72dd1f2e8205190f1",
    ];
    let skk = [
        "e3e4fce65278e1b62d3009763ec2c5f71996a1ac556d3cff3971f98f4d552229fd296a6fb3b0840a0371e950771cfcb0ab78dd47ffd7165046658fe0e6ebff69",
        "3361a20b7dd8981e41774a17904103b391269fe872a6e6c19cd0a91688a05e627c5ceef673eed1c788c0c42443fef0d8768b2970f4850358ec8b77911cd2bca1",
        "abbcd58f380175900aad75aaa00195f069aedf59dc57f7a8cf649c9536995135b8c3c3c1e3ce31ad3396cc1b95663125732113a97dda450c14bdcfff42101894",
        "d0caf2f9bcc9277840672eefd4fdc7c95abbc8b95d36a5533a7332089fc8b21650eea014502f98e678902fea5e385f86c295be9df9685a0c328a5a4cd6257cfb",
        // "18f9260690a1ae01683a955bf23821da4aff977f436313e2affc9457d60f9e73ff8994c927dfc7be0f155b0113f5fa1e2b93059387b22d743fa722c9e8fa1236",
        // "d80b86807f2c0e6ed880211b0710e7092aa87816000452ab3b874bac92dc5a4f5f827e2ece2b4148e4d61da3f9c049a87c5c61331b9d82617115a83f5a4cc735",
        // "2597fdbf713f3686435ba00e16ab07354ed998767f33965e5f549a855bd036b6be9a5490aaf7ec26e9ff509fbe488f35d89355a659f72c041cf1e2e797ba0aa8",
        // "f2946418320a7c212038574350d1880a98a441735e3e02c4a51cb486babe34b5d8c868ee748202290c38f1a176676325423a4491f45d1010fe3ef02ac610727d",
        // "d4aea6b2d828167457708c2f5760e26ad9b318ab12294acd3a1d13de76cb285a7cb1442b2da924d8e1e16d33a853b3bfc16b42b83b7ef1c72dd1f2e8205190f1",
    ];
    let pk = tmelcrypt::Ed25519PK(hex::decode(pkk[i]).unwrap().as_slice().try_into().unwrap());
    let mut sk = [0u8; 64];
    sk.copy_from_slice(&hex::decode(skk[i]).unwrap());
    let sk = tmelcrypt::Ed25519SK(sk);
    (pk, sk)
}
