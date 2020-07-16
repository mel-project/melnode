use crate::smt::dbnode::*;
use parking_lot::{Mutex, RwLock};
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::collections::HashSet;
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
        raw.set_gc_roots(&roots)
    }
    /// Spawns out a tree at the given hash.
    pub fn get_tree(&self, root_hash: tmelcrypt::HashVal) -> Tree {
        // ensure a consistent view of the tree hashes
        let mut trees = self.trees.write();
        trees
            .entry(root_hash)
            .or_insert_with(|| Tree {
                dbm: self.clone(),
                hash: root_hash,
                hack_ctr: Arc::new(()),
            })
            .clone()
    }

    /// Helper function to load a node into memory.
    fn read_cached(&self, hash: tmelcrypt::HashVal) -> DBNode {
        let mut cache = self.cache.write();
        cache
            .entry(hash)
            .or_insert_with(|| self.raw.read().get(hash))
            .clone()
    }

    /// Helper function to write a node into the cache.
    fn write_cached(&self, hash: tmelcrypt::HashVal, value: DBNode) {
        let mut cache = self.cache.write();
        cache.insert(hash, value);
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
pub struct MemDB {
    mapping: HashMap<tmelcrypt::HashVal, DBNode>,
    roots: Vec<tmelcrypt::HashVal>,
    gc_mark: usize,
}

impl RawDB for MemDB {
    fn get(&self, hash: tmelcrypt::HashVal) -> DBNode {
        self.mapping.get(&hash).unwrap_or(&DBNode::Zero).clone()
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
