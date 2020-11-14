use std::{collections::HashSet, sync::Arc, time::Duration};

use melnet::NetState;
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
        nstate.register_verb("gossip", {
            move |nstate, req: GossipMsg| {
                let seen_msgs = seen_msgs.clone();
                async {
                    println!("received {:?}", req);
                    if seen_msgs.read().get(&req.id).is_none() {
                        seen_msgs.write().insert(req.id);
                        // spam to all my neighbors
                        if let Err(e) = spam_neighbors(&nstate, req).await {
                            println!("failed: {}", e);
                        }
                    }
                    drop(seen_msgs);
                    drop(nstate);
                    Ok(())
                }
            }
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
        melnet::g_client()
            .request(neigh, "gossip", "gossip", req.clone())
            .await?;
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
