use crate::smt;
use lmdb::Transaction;
use snap::raw;
use std::error::Error;
use std::time::Instant;

pub struct LMDB {
    env: lmdb::Environment,
    db: lmdb::Database,
}

fn deerror<T>(res: lmdb::Result<T>) -> Option<T> {
    match res {
        Ok(x) => Some(x),
        Err(lmdb::Error::NotFound) => None,
        Err(x) => panic!("fatal LMDB error: {:?}", x),
    }
}

impl LMDB {
    pub fn new(env: lmdb::Environment, name: Option<&str>) -> Result<LMDB, Box<dyn Error>> {
        let db = env.open_db(name)?;
        Ok(LMDB { env, db })
    }
}

impl smt::RawKeyVal for LMDB {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let txn = self
            .env
            .begin_ro_txn()
            .expect("can't start R/O LMDB transaction");
        let snapped = deerror(txn.get(self.db, &key))?;
        Some(raw::Decoder::new().decompress_vec(snapped).unwrap())
    }
    fn set_batch<T>(&mut self, kvv: T)
    where
        T: IntoIterator<Item = (Vec<u8>, Option<Vec<u8>>)>,
    {
        let mut txn = self
            .env
            .begin_rw_txn()
            .expect("can't start R/W/ LMDB transaction");
        let mut count = 0;
        let mut delcount = 0;
        for (k, v) in kvv {
            count += 1;
            //println!("flushing {} to LMDB", hex::encode(&k));
            match v {
                Some(v) => {
                    let snapped = raw::Encoder::new().compress_vec(&v).unwrap();
                    txn.put(self.db, &k, &snapped, lmdb::WriteFlags::empty())
                        .unwrap()
                }
                None => {
                    delcount += 1;
                    txn.del(self.db, &k, None).unwrap_or(());
                }
            }
        }
        let start = Instant::now();
        txn.commit().unwrap();
        println!(
            "committed {} entries ({} deletes) into LMDB within {} secs",
            count,
            delcount,
            start.elapsed().as_secs_f32()
        );
    }
}
