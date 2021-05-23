use novasmt::FullProof;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;
use std::marker::PhantomData;
use tmelcrypt::HashVal;

/// SmtMapping is a type-safe, constant-time cloneable, imperative-style interface to a sparse Merkle tree.
pub struct SmtMapping<K: Serialize, V: Serialize + DeserializeOwned> {
    pub mapping: novasmt::Tree,
    _phantom_k: PhantomData<K>,
    _phantom_v: PhantomData<V>,
}

impl<K: Serialize, V: Serialize + DeserializeOwned> Debug for SmtMapping<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.mapping.root_hash().fmt(f)
    }
}

impl<K: Serialize, V: Serialize + DeserializeOwned> Clone for SmtMapping<K, V> {
    fn clone(&self) -> Self {
        SmtMapping::new(self.mapping.clone())
    }
}

impl<K: Serialize, V: Serialize + DeserializeOwned> SmtMapping<K, V> {
    /// Clears a mapping.
    pub fn clear(&mut self) {
        self.mapping.clear()
    }
    /// Returns true iff the mapping is empty.
    pub fn is_empty(&self) -> bool {
        self.root_hash().0 == [0; 32]
    }
    /// new converts a type-unsafe SMT to a SmtMapping
    pub fn new(tree: novasmt::Tree) -> Self {
        SmtMapping {
            mapping: tree,
            _phantom_k: PhantomData,
            _phantom_v: PhantomData,
        }
    }
    /// get obtains a mapping
    pub fn get(&self, key: &K) -> (Option<V>, FullProof) {
        let key = tmelcrypt::hash_single(&stdcode::serialize(key).unwrap());
        let (v_bytes, proof) = self.mapping.get_with_proof(key.0);
        match v_bytes.len() {
            0 => (None, proof),
            _ => {
                let res: V = stdcode::deserialize(&v_bytes).expect("SmtMapping saw invalid data");
                (Some(res), proof)
            }
        }
    }
    /// insert inserts a mapping, replacing any existing mapping
    pub fn insert(&mut self, key: K, val: V) {
        let key = tmelcrypt::hash_single(&stdcode::serialize(&key).unwrap());
        self.mapping
            .insert(key.0, stdcode::serialize(&val).unwrap().into());
    }
    /// delete deletes a mapping, replacing the mapping with a mapping to the empty bytestring
    pub fn delete(&mut self, key: &K) {
        let key = tmelcrypt::hash_single(&stdcode::serialize(key).unwrap());
        self.mapping.insert(key.0, Default::default());
    }
    /// root_hash returns the root hash
    pub fn root_hash(&self) -> HashVal {
        HashVal(self.mapping.root_hash())
    }
    /// val_iter returns an iterator over the values.
    pub fn val_iter(&'_ self) -> impl Iterator<Item = V> + '_ {
        self.mapping
            .iter()
            .map(|(_, v)| stdcode::deserialize::<V>(&v).unwrap())
    }
}
