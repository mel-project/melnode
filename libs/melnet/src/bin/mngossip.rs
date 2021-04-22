use std::{collections::HashSet, sync::Arc, time::Duration};

use melnet::{NetState, Request};
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use smol::prelude::*;

static EXEC: smol::Executor = smol::Executor::new();

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GossipMsg {
    id: u128,
    body: String,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    smol::block_on(EXEC.run(async {
        // create listener
        let tcp_listener = smol::net::TcpListener::bind("127.0.0.1:0").await?;
        println!("MY ADDRESS: {}", tcp_listener.local_addr()?);
        let nstate = NetState::new_with_name("gossip");
        let seen_msgs = Arc::new(parking_lot::RwLock::new(HashSet::new()));
        // register the verbs
        nstate.listen("gossip", move |req: Request<GossipMsg, _>| {
            println!("received {:?}", req.body);
            if seen_msgs.read().get(&req.body.id).is_none() {
                seen_msgs.write().insert(req.body.id);
                let body = req.body.clone();
                let state = req.state.clone();
                // spam to all my neighbors
                smolscale::spawn(async move {
                    if let Err(e) = spam_neighbors(&state, body).await {
                        println!("failed: {}", e);
                    }
                })
                .detach();
            }
            req.response.send(Ok(()));
        });
        // listen
        nstate
            .run_server(tcp_listener)
            .or(cmd_prompt(&nstate))
            .await;
        Ok(())
    }))
}

async fn spam_neighbors(nstate: &NetState, req: GossipMsg) -> anyhow::Result<()> {
    for &neigh in nstate.routes().iter() {
        melnet::request(neigh, "gossip", "gossip", req.clone()).await?;
    }
    Ok(())
}

async fn cmd_prompt(nstate: &NetState) {
    loop {
        spam_neighbors(
            nstate,
            GossipMsg {
                id: rand::thread_rng().gen(),
                body: "Hello World!".into(),
            },
        )
        .await
        .unwrap();
        smol::Timer::after(Duration::from_secs(10)).await;
    }
}
