mod ondisk;
mod smt;
use crate::smt::*;
use rand::prelude::*;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Instant;

fn main() {
    env_logger::init();
    let db = {
        let lmdb_env = lmdb::Environment::new()
            .set_max_dbs(1)
            .set_map_size(1 << 40)
            .open(Path::new("LMDB_TEST"))
            .unwrap();
        ondisk::LMDB::new(lmdb_env, None).unwrap()
    };
    benchmark("LMDB", DBManager::load(db));
}

fn benchmark(name: &str, db: DBManager) {
    let mut tree = db.get_tree(tmelcrypt::HashVal::default());
    dbg!(tree.root_hash());

    let iterations = 100000;
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
    for (i, (k, v)) in kvv.iter().enumerate() {
        tree = tree.set(*k, v);
        // println!("set {:?}", k);
        assert!(!tree.get(*k).0.is_empty());
        if i % 10000 == 0 {
            db.sync();
        }
    }
    db.sync();
    println!("time: {:.2} sec", insert_start.elapsed().as_secs_f64());
    println!(
        "speed: {:.2} inserts/sec",
        iterations as f64 / insert_start.elapsed().as_secs_f64()
    );
    println!("");
    //println!("{}", db.debug_graphviz());

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
