use crate::common::*;
use crate::storage::Storage;
use anyhow::Result;
use blkstructs::{FinalizedState, Transaction};
use derive_more::*;
use melnet::Request;
//use future_parking_lot::rwlock::{FutureReadable, FutureWriteable};
use crate::client_protocol::*;
use smol::{channel::Sender, net::TcpListener};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use tmelcrypt::HashVal;

// /// A structure representing a running auditor (full node).
// #[derive(Deref, Clone)]
// pub struct Auditor {
//     actor: Arc<Actor<AuditorMsg>>,
// }

// pub enum AuditorMsg {
//     /// Obtain the NetState out of the Auditor.
//     GetNet(Sender<melnet::NetState>),
//     /// Broadcast a transaction onto the network.
//     SendTx(Transaction, Sender<Result<()>>),
//     /// Broadcast a finalized block onto the network. Should only be called after the block is actually finalized!
//     SendFinalizedBlk(FinalizedState, symphonia::QuorumCert),
// }
// use AuditorMsg::*;

// impl Auditor {
//     /// Creates a new Auditor from the given listener.
//     pub async fn new(
//         listener: TcpListener,
//         state: Arc<RwLock<Storage>>,
//         bootstrap: &[SocketAddr],
//     ) -> Result<Self> {
//         let net = new_melnet(&listener, TEST_ANET).await?;
//         for addr in bootstrap {
//             net.add_route(*addr)
//         }
//         let actor = spawn_auditor_actor(listener, state, net);
//         log::info!("auditor actor started!");
//         Ok(Auditor { actor })
//     }
// }

// fn spawn_auditor_actor(
//     listener: TcpListener,
//     storage: Arc<RwLock<Storage>>,
//     net: melnet::NetState,
// ) -> Arc<Actor<AuditorMsg>> {
//     // local fn definitions
//     async fn forward_tx(tx: blkstructs::Transaction, dest: SocketAddr) -> Result<bool> {
//         let raw_result = melnet::g_client()
//             .request(dest, TEST_ANET, "newtx", tx)
//             .await?;
//         Ok(raw_result)
//     }
//     async fn send_blk(
//         state: FinalizedState,
//         cons_proof: symphonia::QuorumCert,
//         dest: SocketAddr,
//     ) -> Result<()> {
//         let txx = &state.inner_ref().transactions;
//         let mut msg = NewBlkRequest {
//             consensus: cons_proof,
//             header: state.header(),
//             txhashes: txx.val_iter().map(|v| v.hash_nosigs()).collect(),
//             partial_transactions: vec![],
//         };
//         // first attempt
//         let first_attempt: NewBlkResponse = melnet::g_client()
//             .request(dest, TEST_ANET, "newblk", msg.clone())
//             .await?;
//         if first_attempt.missing_txhashes.is_empty() {
//             return Ok(());
//         }
//         let mut missing_txhashes = HashSet::new();
//         for txh in first_attempt.missing_txhashes {
//             missing_txhashes.insert(txh);
//         }
//         // second attempt
//         let missing_txx = txx
//             .val_iter()
//             .filter(|v| missing_txhashes.contains(&v.hash_nosigs()))
//             .collect();
//         msg.partial_transactions = missing_txx;
//         let second_attempt: NewBlkResponse = melnet::g_client()
//             .request(dest, TEST_ANET, "newblk", msg)
//             .await?;
//         if !second_attempt.missing_txhashes.is_empty() {
//             return Err(anyhow::anyhow!(
//                 "claimed that txhashes are missing twice in a row"
//             ));
//         }
//         Ok(())
//     }

//     async fn blksync() {
//         // loop {
//         //     // wait 10 seconds between each iteration
//         //     Timer::after(Duration::from_secs(10)).await;
//         //     log::debug!("blksync iteration")
//         // }
//     }

//     // hook up callbacks
//     let auditor_actor = {
//         let net = net.clone();
//         let storage = storage.clone();
//         Arc::new(Actor::spawn(|mut mail| async move {
//             let net2 = net.clone();
//             let _server_runner = smolscale::spawn(async move { net2.run_server(listener).await });
//             let _blksync_runner = smolscale::spawn(blksync());
//             loop {
//                 match mail.recv().await {
//                     GetNet(s) => s.send(net.clone()).await.unwrap(),
//                     SendTx(tx, s) => {
//                         let res = storage.write().insert_tx(tx.clone());
//                         if res.is_ok() {
//                             // hey, it's a good TX! we should tell our friends too!
//                             log::debug!("good tx {:?}, forwarding to 16 peers", tx.hash_nosigs());
//                             for dest in net.routes().into_iter().take(16) {
//                                 let tx = tx.clone();
//                                 smolscale::spawn(async move {
//                                     let _ = forward_tx(tx, dest).await;
//                                 })
//                                 .detach();
//                             }
//                         }
//                         s.send(res).await.unwrap();
//                     }
//                     SendFinalizedBlk(blk, cons_proof) => {
//                         // we only promulgate states we believe in!
//                         assert_eq!(
//                             blk.header().hash(),
//                             storage.read().last_block().unwrap().inner().header().hash()
//                         );
//                         log::debug!(
//                             "promulgating blk height {} to 16 neighbors",
//                             blk.header().height
//                         );
//                         // spam the block to up to 16 neighbors
//                         for dest in net.routes().into_iter().take(16) {
//                             log::debug!("promulgating blk to {}", dest);
//                             let blk = blk.clone();
//                             let cons_proof = cons_proof.clone();
//                             smolscale::spawn(async move {
//                                 let res = send_blk(blk, cons_proof, dest).await;
//                                 log::debug!("result of promulgation {:?}", res.is_ok());
//                             })
//                             .detach();
//                         }
//                     }
//                 }
//             }
//         }))
//     };

//     {
//         let actor = auditor_actor.clone();
//         // handle new transactions
//         net.register_verb(
//             "newtx",
//             melnet::anon_responder(move |req: melnet::Request<Transaction, _>| {
//                 let tx = req.body.clone();
//                 let actor = actor.clone();
//                 smolscale::spawn(async move {
//                     req.respond(Ok(actor.send_ret(|s| SendTx(tx, s)).await.is_ok()))
//                 })
//                 .detach();
//             }),
//         );
//         // handle tx requests
//         let gettx_storage = storage.clone();
//         net.register_verb(
//             "gettx",
//             melnet::anon_responder(move |req: melnet::Request<HashVal, _>| {
//                 let gettx_storage = gettx_storage.clone();
//                 let txid = req.body;
//                 let resp = gettx_storage
//                     .read()
//                     .get_tx(txid)
//                     .ok_or_else(|| melnet::MelnetError::Custom(String::from("no such TX")));
//                 req.respond(resp)
//             }),
//         );
//         // handle new blocks
//         let newblk_storage = storage.clone();
//         let auditor_actor_c = auditor_actor.clone();
//         net.register_verb(
//             "newblk",
//             melnet::anon_responder(move |mreq: melnet::Request<NewBlkRequest, _>| {
//                 let newblk_storage = newblk_storage.clone();
//                 let auditor_actor_c = auditor_actor_c.clone();
//                 let req = mreq.body.clone();
//                 let resp = move || {
//                     let storage = newblk_storage.clone();
//                     let auditor_actor = auditor_actor_c.clone();
//                     let mut storage = storage.write();
//                     let bad_err = || melnet::MelnetError::Custom(String::from("rejected"));
//                     // first we validate the consensus proof
//                     log::warn!("not validating the consensus proof yet!");
//                     let txmap = {
//                         let mut toret = HashMap::new();
//                         for tx in req.partial_transactions {
//                             toret.insert(tx.hash_nosigs(), tx);
//                         }
//                         toret
//                     };
//                     let hash_to_tx = |txh| match storage.get_tx(txh) {
//                         Some(v) => Some(v),
//                         None => txmap.get(&txh).cloned(),
//                     };
//                     // then we check whether we have all the transactions
//                     let missing_hashes: Vec<HashVal> = req
//                         .txhashes
//                         .iter()
//                         .filter(|txh| hash_to_tx(**txh).is_none())
//                         .cloned()
//                         .collect();
//                     log::debug!(
//                         "newblk: {}/{} missing",
//                         missing_hashes.len(),
//                         req.txhashes.len()
//                     );
//                     if !missing_hashes.is_empty() {
//                         // reply to say that we have missing hashes
//                         return Ok(NewBlkResponse {
//                             missing_txhashes: missing_hashes,
//                         });
//                     }
//                     // we don't have missing hashes. time to construct the state
//                     let total_txx: Vec<Transaction> = req
//                         .txhashes
//                         .iter()
//                         .map(|tx| hash_to_tx(*tx).expect("cannot obtain total_txx?!"))
//                         .collect();
//                     let new_blk = blkstructs::Block {
//                         header: req.header,
//                         transactions: total_txx,
//                     };
//                     match storage.apply_block(new_blk) {
//                         Err(_) => Err(bad_err()),
//                         Ok(_) => {
//                             auditor_actor.send(SendFinalizedBlk(
//                                 storage.inner().last_block().unwrap(),
//                                 req.consensus,
//                             ));
//                             Ok(NewBlkResponse {
//                                 missing_txhashes: vec![],
//                             })
//                         }
//                     }
//                 };
//                 mreq.respond(resp());
//             }),
//         );

//         // *********** CLIENT METHODS ***********
//         // get the latest state. TODO THIS IS TOTALLY UNVERIFIABLE
//         let s2 = storage.clone();
//         net.register_verb(
//             "get_latest_header",
//             melnet::anon_responder(move |req: Request<(), _>| {
//                 let storage = s2.read();
//                 if let Some(block) = storage.last_block() {
//                     req.respond(Ok(block.inner().header()))
//                 } else {
//                     req.respond(Err(melnet::MelnetError::Custom(String::from(
//                         "no blocks yet",
//                     ))))
//                 }
//             }),
//         );
//         // handle coin requests
//         let s2 = storage.clone();
//         net.register_verb(
//             "get_coin",
//             melnet::anon_responder(move |req: Request<GetCoinRequest, _>| {
//                 let request = req.body.clone();
//                 let storage = s2.read();
//                 let state = storage.history.get(&request.height);
//                 if let Some(state) = state {
//                     let (coin_data, proof) = state.inner().inner_ref().coins.get(&request.coin_id);
//                     req.respond(Ok(GetCoinResponse {
//                         coin_data,
//                         compressed_proof: proof.compress().0,
//                     }))
//                 } else {
//                     req.respond(Err(melnet::MelnetError::Custom(String::from(
//                         "no such height",
//                     ))));
//                 }
//             }),
//         );
//         // handle transaction requests
//         let s2 = storage;
//         net.register_verb(
//             "get_tx",
//             melnet::anon_responder(move |req: Request<GetTxRequest, _>| {
//                 let request = req.body.clone();
//                 let storage = s2.read();
//                 let state = storage.history.get(&request.height);
//                 if let Some(state) = state {
//                     let (transaction, proof) =
//                         state.inner().inner_ref().transactions.get(&request.txhash);
//                     req.respond(Ok(GetTxResponse {
//                         transaction,
//                         compressed_proof: proof.compress().0,
//                     }))
//                 } else {
//                     req.respond(Err(melnet::MelnetError::Custom(String::from(
//                         "no such height",
//                     ))));
//                 }
//             }),
//         );
//     }
//     auditor_actor
// }
