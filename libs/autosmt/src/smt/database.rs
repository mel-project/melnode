use crate::smt::dbnode::*;
use crate::smt::*;
use genawaiter::sync::Gen;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Wraps around a raw key-value store and produces trees. The main interface to the library.
#[derive(Clone)]
pub struct Forest {
    raw: Arc<RwLock<dyn RawDB>>, // dynamic dispatch for ergonomics
}

impl Forest {
    /// Loads a DBManager from a RawDB
    pub fn load(raw: impl RawDB + 'static) -> Self {
        Forest {
            raw: Arc::new(RwLock::new(raw)),
        }
    }
    /// Spawns out a tree at the given hash.
    pub fn get_tree(&self, root_hash: tmelcrypt::HashVal) -> Tree {
        Tree {
            dbm: self.clone(),
            hash: root_hash,
        }
    }

    /// Helper function to load a node.
    #[must_use]
    pub(crate) fn read(&self, hash: tmelcrypt::HashVal) -> DBNode {
        if hash == tmelcrypt::HashVal::default() {
            return DBNode::Zero;
        }
        let out = self.raw.read().get(hash);
        debug_assert_eq!(out.hash(), hash);
        out
    }

    /// Helper function to write a node into the cache.
    pub(crate) fn write(&self, hash: tmelcrypt::HashVal, value: DBNode) {
        self.raw.write().set_batch(vec![(hash, value)])
    }

    // /// Draws a debug GraphViz representation of the tree.
    // pub fn debug_graphviz(&self) -> String {
    //     let mut output = String::new();
    //     log::debug!("traversal_stack init..");
    //     let mut traversal_stack: Vec<_> = self
    //         .trees
    //         .read()
    //         .iter()
    //         .filter_map(|(k, v)| {
    //             if Arc::strong_count(&v.hack_ctr) != 1 {
    //                 Some(k)
    //             } else {
    //                 None
    //             }
    //         })
    //         .cloned()
    //         .collect();
    //     output.push_str("digraph G {\n");
    //     let mut draw_dbn = |dbn: &DBNode, color: &str| {
    //         let kind = match dbn {
    //             DBNode::Internal(_) => String::from("I"),
    //             DBNode::Data(DataNode {
    //                 level: l, key: k, ..
    //             }) => format!("D-({}, {:?})", l, k),
    //             DBNode::Zero => String::from("Z"),
    //         };
    //         let ptrs = dbn.out_ptrs();
    //         let curr_hash = dbn.hash();
    //         output.push_str(&format!(
    //             "\"{:?}\" [style=filled label=\"{}-{:?}\" fillcolor={} shape=rectangle]\n",
    //             curr_hash, kind, curr_hash, color
    //         ));
    //         for (i, p) in ptrs.into_iter().enumerate() {
    //             if p != tmelcrypt::HashVal::default() {
    //                 output.push_str(&format!(
    //                     "\"{:?}\" -> \"{:?}\" [label={}]\n",
    //                     curr_hash, p, i
    //                 ));
    //             }
    //         }
    //     };
    //     traversal_stack.dedup();
    //     let mut seen = HashSet::new();
    //     while !traversal_stack.is_empty() {
    //         let curr = traversal_stack.pop().unwrap();
    //         log::debug!("traversal_stack {} @ {:?}", traversal_stack.len(), curr);
    //         if curr == tmelcrypt::HashVal::default() {
    //             continue;
    //         }
    //         if !seen.insert(curr) {
    //             log::warn!("cycle detected in SMT on {:?}", curr);
    //             continue;
    //         }
    //         match self.cache.read().get(&curr) {
    //             Some(dbn) => {
    //                 draw_dbn(dbn, "azure");
    //                 traversal_stack.extend_from_slice(&dbn.out_ptrs_nonnull());
    //             }
    //             None => {
    //                 let dbn = self.raw.read().get(curr);
    //                 draw_dbn(&dbn, "white");
    //                 traversal_stack.extend_from_slice(&dbn.out_ptrs_nonnull());
    //             }
    //         }
    //     }
    //     output.push_str("}\n");
    //     output
    // }
}

#[derive(Clone)]
pub struct Tree {
    dbm: Forest,
    hash: tmelcrypt::HashVal,
}

impl Tree {
    /// Helper function to get DBNode representation.
    fn to_dbnode(&self) -> DBNode {
        self.dbm.read(self.hash)
    }

    /// Gets a binding and its proof.
    pub fn get(&self, key: tmelcrypt::HashVal) -> (Vec<u8>, FullProof) {
        let dbn = self.to_dbnode();
        let (bind, mut proof) = dbn.get_by_path_rev(&merk::key_to_path(key), key, &self.dbm);
        proof.reverse();
        (bind, FullProof(proof))
    }

    /// Sets a binding, obtaining a new tree.
    #[must_use]
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

    /// Zeroed version.
    pub fn zeroed(&self) -> Self {
        self.dbm.get_tree(tmelcrypt::HashVal::default())
    }

    /// Iterator.
    pub fn iter(&self) -> impl Iterator<Item = (tmelcrypt::HashVal, Vec<u8>)> {
        // DFS of the entire tree in arbitrary order. Generator returns only the data bindings.
        let mut stack = vec![self.to_dbnode()];
        let dbm = self.dbm.clone();
        let gen = Gen::new(|co| async move {
            while !stack.is_empty() {
                let top = stack.pop().unwrap();
                match &top {
                    DBNode::Internal(_) => {
                        let next = top
                            .out_ptrs()
                            .into_iter()
                            .map(|h| dbm.read(h))
                            .collect::<Vec<_>>();
                        stack.extend_from_slice(&next);
                    }
                    DBNode::Data(dat) => co.yield_((dat.key, dat.data.clone())).await,
                    DBNode::Zero => continue,
                }
            }
        });
        gen.into_iter()
    }
}

/// Represents a raw key-value store, similar to that offered by key-value databases. Internally manages garbage collection.
pub trait RawDB: Send + Sync {
    /// Gets a database node given its hash.
    fn get(&self, hash: tmelcrypt::HashVal) -> DBNode;
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
        self.mapping
            .get(&hash)
            .unwrap_or_else(|| panic!("failed to get: {:?}", hash))
            .clone()
    }

    fn set_batch(&mut self, kvv: Vec<(tmelcrypt::HashVal, DBNode)>) {
        for (k, v) in kvv {
            self.mapping.insert(k, v);
        }
        // self.gc();
    }

    fn set_gc_roots(&mut self, roots: &[tmelcrypt::HashVal]) {
        self.roots = roots.to_owned();
        log::debug!("set_gc_roots({})", roots.len());
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
            let mut seen = HashSet::new();
            while !stack.is_empty() {
                let curr = stack.pop().unwrap();
                if curr == tmelcrypt::HashVal::default() {
                    continue;
                }
                let existing = self.get(curr);
                new_mapping.insert(curr, existing.clone());
                for outptr in existing.out_ptrs() {
                    if !seen.contains(&outptr) {
                        stack.push(outptr);
                        seen.insert(outptr);
                    }
                }
                log::trace!("GC copying (stack length {})", stack.len())
            }
            log::debug!(
                "len {} > gc_mark {}, gcing -> {}",
                self.mapping.len(),
                self.gc_mark,
                new_mapping.len()
            );
            // replace the mapping
            self.mapping = new_mapping;
            self.gc_mark = self.mapping.len() * 2
        }
    }
}

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn test_something() {}
}
