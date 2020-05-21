fn main() {
    let db = smart_smt::TrivialDB::new();
    let tree = smart_smt::Tree::new(&smart_smt::wrap_db(db));
    let tree = tree.set(smart_smt::hash::datablock(b"hello"), b"world");
    println!("root has hash {}", hex::encode(tree.root_hash()));
}
