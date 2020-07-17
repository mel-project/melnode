mod ondisk;
mod smt;
use crate::smt::*;
use rand::prelude::*;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Instant;
fn main() {
    let mut top_hash: [u8; 32] = [0; 32];
    hex::decode_to_slice(
        "db2e646567c3c90d0a029efa31e5c60b9bade1340501b3072d12e71018552d7b",
        &mut top_hash,
    )
    .unwrap();

    benchmark("MEMO", DBManager::load(MemDB::default()));
}

fn benchmark(name: &str, db: DBManager) {
    let mut tree = db.get_tree(tmelcrypt::HashVal::default());

    let iterations = 10000;
    let kvv: Vec<(tmelcrypt::HashVal, Vec<u8>)> = {
        let mut kvv = Vec::new();
        for i in 0..iterations {
            let mut v = b"hello".to_vec();
            kvv.push((tmelcrypt::hash_single(&(i as u64).to_be_bytes()), v));
        }
        kvv
    };

    println!("*** INSERT ***");
    let insert_start = Instant::now();
    for (_, (k, v)) in kvv.iter().enumerate() {
        tree = tree.set(*k, v);
        // println!("set {:?}", k);
        assert!(!tree.get(*k).0.is_empty())
    }
    println!("time: {:.2} sec", insert_start.elapsed().as_secs_f64());
    println!(
        "speed: {:.2} inserts/sec",
        iterations as f64 / insert_start.elapsed().as_secs_f64()
    );
    println!("");
    println!("{}", db.debug_graphviz());

    println!("*** READ ***");
    let mut proof_sizes = 0;
    let retrieval_start = Instant::now();
    for (i, (k, v)) in kvv.iter().enumerate() {
        let (vv, p) = tree.get(*k);
        assert_eq!(vv, v.clone());
        proof_sizes += p.compress().0.len();
    }
    println!("time: {:.2} sec", retrieval_start.elapsed().as_secs_f64());
    println!(
        "speed: {:.2} reads/sec",
        iterations as f64 / retrieval_start.elapsed().as_secs_f64()
    );
    println!(
        "average proof length: {:.2} bytes",
        proof_sizes as f64 / iterations as f64
    );
    println!("")
}
