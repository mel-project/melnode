/// A trait that all database backends must implement.
pub trait DbBackend: Send + Sync + 'static {
    /// Inserts a key-value pair, returning the previous value if it exists.
    fn insert(&mut self, key: &[u8], value: &[u8]) -> Option<Vec<u8>>;
    /// Deletes a key-value pair by the key, returning the previous value if it exists.
    fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>>;
    /// Gets a value by the key.
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    /// Iterates over a range of keys, returning a vector of actually existing keys.
    fn key_range(&self, start: &[u8], end: &[u8]) -> Vec<Vec<u8>>;
}
