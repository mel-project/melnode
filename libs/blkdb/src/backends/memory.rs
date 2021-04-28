use crate::traits::DbBackend;

/// An in-memory DbBackend.
#[derive(Default, Debug)]
pub struct InMemoryBackend {
    inner: im::OrdMap<Vec<u8>, Vec<u8>>,
}

impl DbBackend for InMemoryBackend {
    fn insert(&mut self, key: &[u8], value: &[u8]) -> Option<Vec<u8>> {
        self.inner.insert(key.to_vec(), value.to_vec())
    }
    fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        self.inner.remove(key)
    }
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.inner.get(key).cloned()
    }
    fn key_range(&self, start: &[u8], end: &[u8]) -> Vec<Vec<u8>> {
        self.inner
            .range(start.to_vec()..=end.to_vec())
            .filter(|(k, _)| k.as_slice() <= end && k.as_slice() >= start)
            .map(|(k, _)| {
                assert!(k.as_slice() >= start);
                assert!(k.as_slice() <= end);
                k.clone()
            })
            .collect()
    }
}
