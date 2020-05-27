use crate::smt;

pub struct RocksDB {
    db: rocksdb::DB,
}

impl RocksDB {
    pub fn new(db: rocksdb::DB) -> Self {
        RocksDB { db }
    }
}

impl smt::RawKeyVal for RocksDB {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        match self.db.get(key) {
            Ok(x) => x,
            Err(e) => {
                panic!("rocksdb err {}", e);
            }
        }
    }
    fn set_batch<T>(&mut self, kvv: T)
    where
        T: IntoIterator<Item = (Vec<u8>, Option<Vec<u8>>)>,
    {
        let start = std::time::Instant::now();
        let mut batch = rocksdb::WriteBatch::default();
        let mut count = 0;
        for (k, v) in kvv {
            count += 1;
            match v {
                Some(v) => batch.put(&k, &v),
                None => batch.delete(&k),
            }
        }
        self.db.write(batch).expect("rocksdb can't write batch");
        println!(
            "committed {} entries into RocksDB within {} secs",
            count,
            start.elapsed().as_secs_f32()
        );
    }
}
