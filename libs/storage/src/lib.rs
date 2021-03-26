use std::marker::PhantomData;

use serde::{de::DeserializeOwned, Serialize};

/// A sled-backed mapping.
pub struct SledMap<K: Serialize, V: DeserializeOwned + Serialize> {
    disk_tree: sled::Tree,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

/// TODO: a ToBytes trait so that we can use SledMap with `State`

impl<K: Serialize, V: DeserializeOwned + Serialize> SledMap<K, V> {
    /// Creates a new SledMap.
    pub fn new(disk_tree: sled::Tree) -> Self {
        Self {
            disk_tree,
            _k: PhantomData::default(),
            _v: PhantomData::default(),
        }
    }

    /// Inserts into the SledMap.
    pub fn insert(&self, key: K, value: V) {
        self.disk_tree
            .insert(
                stdcode::serialize(&key).unwrap(),
                stdcode::serialize(&value).unwrap(),
            )
            .expect("insertion failed");
    }

    /// Gets from the SledMap.
    pub fn get(&self, key: &K) -> Option<V> {
        self.disk_tree
            .get(&stdcode::serialize(key).unwrap())
            .expect("get failed")
            .map(|v| stdcode::deserialize(&v).expect("cannot deserialize"))
    }

    /// Removes from the SledMap.
    pub fn remove(&self, key: &K) {
        self.disk_tree
            .remove(&stdcode::serialize(key).unwrap())
            .expect("cannot remove");
    }
}
