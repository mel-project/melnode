use crate::smt::dbnode::*;
use crate::smt::*;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::Arc;

/// Wraps around a raw key-value store and produces trees. The main interface to the library.
#[derive(Clone)]
pub struct DBManager {
    raw: Arc<RwLock<dyn RawDB>>, // dynamic dispatch for ergonomics
    cache: Arc<RwLock<HashMap<tmelcrypt::HashVal, DBNode>>>,
    trees: Arc<RwLock<HashMap<tmelcrypt::HashVal, Tree>>>,
}

impl DBManager {
    /// Loads a DBManager from a RawDB, while not changing the GC roots. To get the roots out, *immediately* query them with get_tree. Otherwise, they'll be lost on the next sync.
    pub fn load(raw: impl RawDB + 'static) -> Self {
        let roots = raw.get_gc_roots();
        let mut cache = HashMap::new();
        for r in roots {
            cache.insert(r, raw.get(r));
        }
        DBManager {
            raw: Arc::new(RwLock::new(raw)),
            cache: Arc::new(RwLock::new(cache)),
            trees: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    /// Syncs the information into the database. DBManager is guaranteed to only sync to database when sync is called.
    pub fn sync(&self) {
        let mut trees = self.trees.write();
        let mut raw = self.raw.write();
        // sync cached info
        let mut kvv = Vec::new();
        for (k, v) in self.cache.write().drain() {
            kvv.push((k, v))
        }
        log::debug!("sync cache of {}", kvv.len());
        raw.set_batch(kvv);
        // sync roots
        let mut roots = Vec::new();
        let mut newtrees = HashMap::new();
        for (k, mut v) in trees.drain() {
            if Arc::get_mut(&mut v.hack_ctr).is_none() {
                roots.push(k)
            } else {
                newtrees.insert(k, v);
            }
        }
        *trees = newtrees;
        log::debug!("sync roots of {}", roots.len());
        raw.set_gc_roots(&roots)
    }
    /// Spawns out a tree at the given hash.
    pub fn get_tree(&self, root_hash: tmelcrypt::HashVal) -> Tree {
        // ensure a consistent view of the tree hashes
        let mut trees = self.trees.write();
        let tree = trees
            .entry(root_hash)
            .or_insert_with(|| Tree {
                dbm: self.clone(),
                hash: root_hash,
                hack_ctr: Arc::new(()),
            })
            .clone();
        debug_assert_eq!(root_hash, tree.root_hash());
        // load into cache
        tree.to_dbnode();
        tree
    }

    /// Helper function to load a node into memory.
    pub(crate) fn read_cached(&self, hash: tmelcrypt::HashVal) -> DBNode {
        if hash == tmelcrypt::HashVal::default() {
            return DBNode::Zero;
        }
        let mut cache = self.cache.write();
        cache
            .entry(hash)
            .or_insert_with(|| self.raw.read().get(hash))
            .clone()
    }

    /// Helper function to write a node into the cache.
    pub(crate) fn write_cached(&self, hash: tmelcrypt::HashVal, value: DBNode) {
        let mut cache = self.cache.write();
        cache.insert(hash, value);
    }

    /// Draws a debug GraphViz representation of the tree.
    pub fn debug_graphviz(&self) -> String {
        let mut output = String::new();
        let mut traversal_stack: Vec<_> = self
            .trees
            .read()
            .iter()
            .filter_map(|(k, v)| {
                if Arc::strong_count(&v.hack_ctr) != 1 {
                    Some(k)
                } else {
                    None
                }
            })
            .cloned()
            .collect();
        output.push_str("digraph G {\n");
        let mut draw_dbn = |dbn: &DBNode, color: &str| {
            let kind = match dbn {
                DBNode::Internal(_) => String::from("I"),
                DBNode::Data(DataNode {
                    level: l, key: k, ..
                }) => format!("D-({}, {:?})", l, k),
                DBNode::Zero => String::from("Z"),
            };
            let ptrs = dbn.out_ptrs();
            let curr_hash = dbn.hash();
            output.push_str(&format!(
                "\"{:?}\" [style=filled label=\"{}-{:?}\" fillcolor={} shape=rectangle]\n",
                curr_hash, kind, curr_hash, color
            ));
            for (i, p) in ptrs.into_iter().enumerate() {
                if p != tmelcrypt::HashVal::default() {
                    output.push_str(&format!(
                        "\"{:?}\" -> \"{:?}\" [label={}]\n",
                        curr_hash, p, i
                    ));
                }
            }
        };
        while !traversal_stack.is_empty() {
            let curr = traversal_stack.pop().unwrap();
            if curr == tmelcrypt::HashVal::default() {
                continue;
            }
            match self.cache.read().get(&curr) {
                Some(dbn) => {
                    draw_dbn(dbn, "azure");
                    traversal_stack.extend_from_slice(&dbn.out_ptrs());
                }
                None => {
                    let dbn = self.raw.read().get(curr);
                    draw_dbn(&dbn, "white");
                    traversal_stack.extend_from_slice(&dbn.out_ptrs());
                }
            }
        }
        output.push_str("}\n");
        output
    }
}

#[derive(Clone)]
pub struct Tree {
    dbm: DBManager,
    hash: tmelcrypt::HashVal,
    hack_ctr: Arc<()>,
}

impl Tree {
    /// Helper function to get DBNode representation.
    fn to_dbnode(&self) -> DBNode {
        self.dbm.read_cached(self.hash)
    }

    /// Gets a binding and its proof.
    pub fn get(&self, key: tmelcrypt::HashVal) -> (Vec<u8>, FullProof) {
        let dbn = self.to_dbnode();
        let (bind, mut proof) = dbn.get_by_path_rev(&merk::key_to_path(key), key, &self.dbm);
        proof.reverse();
        (bind, FullProof(proof))
    }

    /// Sets a binding, obtaining a new tree.
    pub fn set(&self, key: tmelcrypt::HashVal, val: &[u8]) -> Tree {
        let dbn = self
            .to_dbnode()
            .set_by_path(&merk::key_to_path(key), key, val, &self.dbm);
        self.dbm.get_tree(dbn.hash())
    }

    /// Root hash.
    pub fn root_hash(&self) -> tmelcrypt::HashVal {
        self.to_dbnode().hash()
    }
}

/// Represents a raw key-value store, similar to that offered by key-value databases. Internally manages garbage collection.
pub trait RawDB: Send + Sync {
    /// Gets a database node given its hash.
    fn get(&self, hash: tmelcrypt::HashVal) -> DBNode;
    /// Stores a database node.
    fn set(&mut self, hash: tmelcrypt::HashVal, val: DBNode) {
        self.set_batch(vec![(hash, val)]);
    }
    /// Sets a batch of database nodes.
    fn set_batch(&mut self, kvv: Vec<(tmelcrypt::HashVal, DBNode)>);
    /// Sets roots for garbage collection. For correctness, garbage collection *must* only occur while this function is running. This is because, nodes pointed to by the roots might be written before the roots are set.
    /// Both reference-counting and incremental copying GC are pretty easy to implement because "pointers" never mutate.
    fn set_gc_roots(&mut self, roots: &[tmelcrypt::HashVal]);
    /// Gets garbage-collection roots.
    fn get_gc_roots(&self) -> Vec<tmelcrypt::HashVal>;
}

/// A trivial, in-memory RawDB.
#[derive(Default)]
pub struct MemDB {
    mapping: HashMap<tmelcrypt::HashVal, DBNode>,
    roots: Vec<tmelcrypt::HashVal>,
    gc_mark: usize,
}

impl RawDB for MemDB {
    fn get(&self, hash: tmelcrypt::HashVal) -> DBNode {
        self.mapping.get(&hash).unwrap().clone()
    }

    fn set_batch(&mut self, kvv: Vec<(tmelcrypt::HashVal, DBNode)>) {
        for (k, v) in kvv {
            self.mapping.insert(k, v);
        }
    }

    fn set_gc_roots(&mut self, roots: &[tmelcrypt::HashVal]) {
        self.roots = roots.to_owned();
        self.gc()
    }

    fn get_gc_roots(&self) -> Vec<tmelcrypt::HashVal> {
        self.roots.clone()
    }
}

impl MemDB {
    fn gc(&mut self) {
        if self.mapping.len() > self.gc_mark {
            // trivial copying GC
            let mut new_mapping = HashMap::new();
            let mut stack = self.roots.clone();
            // start from the roots
            while !stack.is_empty() {
                let curr = stack.pop().unwrap();
                if curr == tmelcrypt::HashVal::default() {
                    continue;
                }
                let existing = self.get(curr);
                new_mapping.insert(curr, existing.clone());
                for outptr in existing.out_ptrs() {
                    stack.push(outptr)
                }
            }
            // replace the mapping
            self.mapping = new_mapping;
            self.gc_mark = self.mapping.len() * 2
        }
    }
}
