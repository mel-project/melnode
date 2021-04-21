use autosmt::DBNode;
use tmelcrypt::HashVal;

/// A sled-backed `autosmt` database.
pub struct SledTreeDB {
    disk_tree: sled::Tree,
}

impl SledTreeDB {
    /// Creates a new SledTreeDB based on a tree.
    pub fn new(disk_tree: sled::Tree) -> Self {
        Self { disk_tree }
    }
} 

const GC_ROOTS_KEY: &[u8] = b"GC_ROOTS";

impl autosmt::RawDB for SledTreeDB {
    fn get(&self, hash: HashVal) -> DBNode {
        DBNode::from_bytes(
            &self
                .disk_tree
                .get(&hash)
                .expect("sled failed to read")
                .expect("sled didn't have the node we wanted"),
        )
    }

    fn set_batch(&mut self, kvv: Vec<(tmelcrypt::HashVal, DBNode)>) {
        let mut batch = sled::Batch::default();
        for (k, v) in kvv {
            batch.insert(&k.0, v.to_bytes());
        }
        self.disk_tree
            .apply_batch(batch)
            .expect("sled failed to set batch");
    }

    fn set_gc_roots(&mut self, roots: &[tmelcrypt::HashVal]) {
        self.disk_tree
            .insert(
                GC_ROOTS_KEY,
                stdcode::serialize(&roots).expect("could not serialize roots"),
            )
            .expect("sled failed to set gc roots");
        // TODO: actually garbage-collect
    }

    fn get_gc_roots(&self) -> Vec<tmelcrypt::HashVal> {
        self.disk_tree
            .get(GC_ROOTS_KEY)
            .expect("sled failed to get gc roots")
            .map(|v| stdcode::deserialize(&v).expect("gc roots contained garbage"))
            .unwrap_or_default()
    }
}
