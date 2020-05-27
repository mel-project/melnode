fn main() {
    let db = autosmt::TrivialDB::new();
    let tree = autosmt::Tree::new(&autosmt::wrap_db(db));
    let tree = tree.set(autosmt::hash::index(b"hello"), b"world");
    println!("root has hash {}", hex::encode(tree.root_hash()));
}
