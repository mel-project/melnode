use crate::smt::*;
use bitvec::prelude::*;
use parking_lot::RwLock;
use std::fmt::Debug;
use std::sync::Arc;

pub struct Tree<T: database::Database> {
    root: Arc<dbnode::DBNode<T>>,
}

impl<T: database::Database> Clone for Tree<T> {
    fn clone(&self) -> Self {
        Tree {
            root: self.root.clone(),
        }
    }
}

impl<T: database::Database> Tree<T> {
    /// Creates a reference to the empty tree, given a database.
    pub fn new(db: &Arc<RwLock<T>>) -> Self {
        Tree::new_from_hash(db, [0; 32])
    }

    /// Creates a reference to the tree pointed to by the given hash, given a database.
    pub fn new_from_hash(db: &Arc<RwLock<T>>, hash: [u8; 32]) -> Self {
        Tree {
            root: Arc::new(dbnode::DBNode::Internal(
                dbnode::InternalNode::new_from_hash(Arc::clone(db), 256, hash),
            )),
        }
    }

    /// Sets the binding for a key, returning a new tree.
    pub fn set(&self, key: [u8; 32], value: &[u8]) -> Self {
        Tree {
            root: Arc::new(self.root.set_by_path(key, &merk::key_to_path(key), value)),
        }
    }
    /// Obtains the binding for a key, returning the proof.
    pub fn get(&self, key: [u8; 32]) -> (Option<Vec<u8>>, FullProof) {
        let path = merk::key_to_path(key);
        let (vec, proof) = self.root.get_by_path_rev(&path, key);
        let vec = if vec.is_empty() { vec![] } else { vec };
        let proof = proof.into_iter().rev().collect();
        if vec.is_empty() {
            (None, FullProof(proof))
        } else {
            (Some(vec), FullProof(proof))
        }
    }

    /// Gets the root hash.
    pub fn root_hash(&self) -> [u8; 32] {
        self.root.hash()
    }
}

pub struct FullProof(pub Vec<[u8; 32]>);

impl FullProof {
    pub fn compress(&self) -> CompressedProof {
        let FullProof(proof_nodes) = self;
        assert_eq!(proof_nodes.len(), 256);
        // build bitmap
        let mut bitmap = bitvec![Msb0, u8; 0; 256];
        for (i, pn) in proof_nodes.iter().enumerate() {
            if *pn == [0; 32] {
                bitmap.set(i, true);
            }
        }
        let mut bitmap_slice = bitmap.as_slice().to_vec();
        for pn in proof_nodes.iter() {
            if *pn != [0; 32] {
                bitmap_slice.append(&mut pn.to_vec());
            }
        }
        CompressedProof(bitmap_slice)
    }
}

impl std::fmt::Display for FullProof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hexa: Vec<String> = self.0.iter().map(hex::encode).collect();
        hexa.fmt(f)
    }
}

pub struct CompressedProof(pub Vec<u8>);

impl std::fmt::Display for CompressedProof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str: String = hex::encode(&self.0);
        std::fmt::Display::fmt(&str, f)
    }
}
