use std::sync::Arc;

use crate::storage::Storage;

pub struct CoinIndexer {
    // a background task
    _index_task: smol::Task<()>,

    // the internal indexer
    indexer: Arc<melblkidx::Indexer>,
}

impl CoinIndexer {
    /// Creates a new CoinIndexer that pulls from the given storage.
    pub fn new(storage: Storage) -> Self {
        let indexer = Arc::new(melblkidx::Indexer::new(path, client))
        let _index_task = smolscale::spawn(coin_index_loop(storage, indexer))
    }
}
