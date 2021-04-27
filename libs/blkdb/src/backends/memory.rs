use std::ops::RangeBounds;

use parking_lot::RwLock;

use crate::traits::DbBackend;

/// An in-memory DbBackend.
#[derive(Default, Debug)]
pub struct InMemoryBackend {
    inner: RwLock<im::OrdMap<Vec<u8>, Vec<u8>>>,
}

impl DbBackend for InMemoryBackend {
    fn insert(&self, key: &[u8], value: &[u8]) -> Option<Vec<u8>> {
        self.inner.write().insert(key.to_vec(), value.to_vec())
    }
    fn remove(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.inner.write().remove(key)
    }
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.inner.read().get(key).cloned()
    }
    fn key_range(&self, range: impl RangeBounds<[u8]>) -> Vec<Vec<u8>> {
        self.inner
            .read()
            .range(range)
            .map(|(k, _)| k.clone())
            .collect()
    }
}
