use std::{borrow::Borrow, marker::PhantomData};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// A sled-backed mapping.
pub struct SledMap<K: DeserializeOwned + Serialize, V: DeserializeOwned + Serialize> {
    disk_tree: sled::Tree,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

/// TODO: a ToBytes trait so that we can use SledMap with `State`

impl<K: DeserializeOwned + Serialize, V: DeserializeOwned + Serialize> SledMap<K, V> {
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
    pub fn get<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Serialize + ?Sized,
    {
        self.disk_tree
            .get(&stdcode::serialize(&key).unwrap())
            .expect("get failed")
            .map(|v| stdcode::deserialize(&v).expect("cannot deserialize"))
    }

    /// Removes from the SledMap.
    pub fn remove(&self, key: &K) {
        self.disk_tree
            .remove(&stdcode::serialize(key).unwrap())
            .expect("cannot remove");
    }

    // Gets a deserialized iterator with key value pair item
    pub fn get_all(&self) -> impl Iterator<Item = (K, V)> {
        self.disk_tree.iter().map(|t| {
            let (k, v) = t.expect("internal storage error");
            let k = stdcode::deserialize(&k).expect("cannot deserialize");
            let v = stdcode::deserialize(&v).expect("cannot deserialize");
            (k, v)
        })
    }
}
