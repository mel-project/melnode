use novasmt::ContentAddrStore;

/// A meshanina-backed autosmt backend
pub struct MeshaCas {
    inner: meshanina::Mapping,
}

impl MeshaCas {
    /// Takes exclusively ownership of a Meshanina database and creates an autosmt backend.
    pub fn new(db: meshanina::Mapping) -> Self {
        Self { inner: db }
    }

    /// Syncs to disk.
    pub fn flush(&self) {
        self.inner.flush()
    }
}

impl ContentAddrStore for MeshaCas {
    fn get<'a>(&'a self, key: &[u8]) -> Option<std::borrow::Cow<'a, [u8]>> {
        Some(std::borrow::Cow::Owned(
            self.inner.get(tmelcrypt::hash_single(key).0)?.to_vec(),
        ))
    }

    fn insert(&self, key: &[u8], value: &[u8]) {
        self.inner.insert(tmelcrypt::hash_single(key).0, value);
    }
}
