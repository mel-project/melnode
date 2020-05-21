mod ondisk;
mod smt;
use crate::smt::*;
use rand::prelude::*;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Instant;
fn main() {
    let mut top_hash: [u8; 32] = [0; 32];
    hex::decode_to_slice(
        "db2e646567c3c90d0a029efa31e5c60b9bade1340501b3072d12e71018552d7b",
        &mut top_hash,
    )
    .unwrap();

    let lmdb_db = {
        let lmdb_env = lmdb::Environment::new()
            .set_max_dbs(1)
            .set_map_size(1 << 40)
            .open(Path::new("LMDB_TEST"))
            .unwrap();
        let db = ondisk::LMDB::new(lmdb_env, None).unwrap();
        let db = CacheDatabase::new(db);
        db
    };
    // let rocksdb_db = {
    //     let db = rocksdb::DB::open_default("ROCKSDB_TEST").unwrap();
    //     let db = ondisk::RocksDB::new(db);
    //     CacheDatabase::new(db)
    // };
    benchmark("LMDB", lmdb_db);
    //benchmark("RocksDB", rocksdb_db);
}

fn benchmark<T: PersistentDatabase>(name: &str, db: T) {
    let mut rng = thread_rng();
    let db = Arc::new(RwLock::new(db));
    let root_hash = db.read().unwrap().get_persist(0).unwrap_or([0; 32]);
    println!(
        "[[[ {} continuing from root hash {} ]]] \n",
        name,
        hex::encode(root_hash)
    );
    let mut tree = Tree::new_from_hash(&db, root_hash);

    let iterations = 100000;

    let kvv: Vec<([u8; 32], Vec<u8>)> = {
        let mut kvv = Vec::new();
        for _ in 0..iterations {
            let mut k: [u8; 32] = [0; 32];
            let mut v = vec![0; 128];
            rng.fill_bytes(k.as_mut());
            rng.fill_bytes(&mut v);
            kvv.push((k, v));
        }
        kvv
    };

    println!("*** INSERT ***");
    let insert_start = Instant::now();
    for (i, (k, v)) in kvv.iter().enumerate() {
        // if i % 100 == 0 {
        //     println!("{} / {}", i, iterations);
        // }
        tree = tree.set(*k, v);
    }
    db.write().unwrap().set_persist(0, tree.root_hash());
    db.write().unwrap().sync();
    println!("time: {:.2} sec", insert_start.elapsed().as_secs_f64());
    println!(
        "speed: {:.2} inserts/sec",
        iterations as f64 / insert_start.elapsed().as_secs_f64()
    );
    println!(
        "{} hashes/entry",
        hash::HASH_COUNT.with(|hc| hc.borrow().clone()) as f64 / iterations as f64
    );
    println!("");

    println!("*** READ ***");
    let mut proof_sizes = 0;
    let retrieval_start = Instant::now();
    for (k, v) in kvv.iter() {
        let (vv, p) = tree.get(*k);
        assert_eq!(vv.unwrap(), v.clone());
        proof_sizes = proof_sizes + p.compress().0.len();
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
