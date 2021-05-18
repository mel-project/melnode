use autosmt::{Forest, MemDB};
use criterion::{criterion_group, criterion_main, Criterion};
use nanorand::WyRand;
use nanorand::RNG;
use sled_tree::SledTreeDB;
use tmelcrypt::HashVal;
mod sled_tree;

fn inmemory_insert(n: u64) {
    let forest = Forest::load(MemDB::default());
    let mut tree = forest.get_tree(HashVal::default());
    let mut rng = WyRand::new();
    for _ in 0..n {
        let block = (0..1000)
            .map(|_| rng.generate_range(0u8, 255u8))
            .collect::<Vec<_>>();
        let key = rng.rand();
        tree = tree.set(tmelcrypt::hash_single(key), &block);
    }
}

fn sled_insert(n: u64) {
    let forest = Forest::load(SledTreeDB::new(
        sled::open("/tmp/autosmt-criterion")
            .unwrap()
            .open_tree(b"test")
            .unwrap(),
    ));
    let mut tree = forest.get_tree(HashVal::default());
    let mut rng = WyRand::new();
    for _ in 0..n {
        let key = rng.rand();
        tree = tree.set(tmelcrypt::hash_single(key), &key);
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("inmemory_insert 1000", |b| b.iter(|| inmemory_insert(1000)));
    c.bench_function("sled_insert 1000", |b| b.iter(|| sled_insert(1000)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
