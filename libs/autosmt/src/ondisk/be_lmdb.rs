use crate::smt;
use bloomfilter::Bloom;
use lmdb::Cursor;
use lmdb::Transaction;
use snap::raw;
use std::sync::Arc;
use std::time::Instant;
use std::{collections::HashSet, error::Error};
use std::{collections::VecDeque, convert::TryInto};

pub struct LMDB {
    env: Arc<lmdb::Environment>,
    db: lmdb::Database,
}

impl LMDB {
    pub fn new(env: Arc<lmdb::Environment>, name: Option<&str>) -> Result<LMDB, Box<dyn Error>> {
        let db = env.open_db(name)?;
        Ok(LMDB { env, db })
    }
}

impl smt::RawDB for LMDB {
    fn get(&self, key: tmelcrypt::HashVal) -> smt::DBNode {
        if key == tmelcrypt::HashVal::default() {
            return smt::DBNode::Zero;
        }
        let txn = self.ro();
        smt::DBNode::from_bytes(
            &raw::Decoder::new()
                .decompress_vec(txn.get(self.db, &key).expect("LMDB error"))
                .unwrap(),
        )
    }
    fn set_batch(&mut self, kvv: Vec<(tmelcrypt::HashVal, smt::DBNode)>) {
        let mut txn = self.rw();
        for (k, v) in kvv {
            //println!("flushing {} to LMDB", hex::encode(&k));
            let bts = v.to_bytes();
            let snapped = raw::Encoder::new().compress_vec(&bts).unwrap();
            txn.put(self.db, &k, &snapped, lmdb::WriteFlags::empty())
                .unwrap()
        }
        txn.commit().unwrap();
    }

    fn set_gc_roots(&mut self, roots: &[tmelcrypt::HashVal]) {
        let gc_mark = {
            let mut txn = self.rw();
            let mut total = Vec::new();
            for r in roots {
                total.extend_from_slice(&r)
            }
            txn.put(self.db, b"gc_roots", &total, lmdb::WriteFlags::empty())
                .unwrap();
            // check whether we need a GC
            let mk = u32::from_be_bytes(
                txn.get(self.db, b"gc_mark")
                    .unwrap_or(&[0, 0, 0, 0])
                    .try_into()
                    .unwrap(),
            );
            txn.commit().unwrap();
            mk
        };
        let curr_entries = self.env.stat().unwrap().entries() as u32;
        log::debug!("gc: curr_entries {}, gc_mark {}", curr_entries, gc_mark);
        if curr_entries > gc_mark {
            self.run_gc()
        }
    }

    fn get_gc_roots(&self) -> Vec<tmelcrypt::HashVal> {
        let txn = self.ro();
        gc_roots_helper(&txn, self.db)
    }
}

impl LMDB {
    fn rw(&self) -> lmdb::RwTransaction {
        self.env
            .begin_rw_txn()
            .expect("can't start R/W LMDB transaction")
    }

    fn ro(&self) -> lmdb::RoTransaction {
        self.env
            .begin_ro_txn()
            .expect("can't start R/O LMDB transaction")
    }

    fn run_gc(&mut self) {
        let env = self.env.clone();
        let db = self.db;
        // we start the GC now
        let mut txn = env
            .begin_rw_txn()
            .expect("can't start R/W LMDB transaction");
        let roots = gc_roots_helper(&txn, db);
        // MARK phase
        // TODO on-disk marking data structure
        // Imprecise bloomfilter is totally fine
        let mut marked = Bloom::new_for_fp_rate(env.stat().unwrap().entries(), 0.1);
        let mut queue: VecDeque<_> = roots.into();
        let mut enqueued = HashSet::new();
        let mut mark_ctr = 0;
        let mut sweep_ctr = 0;
        let start = Instant::now();
        while !queue.is_empty() {
            let top = queue.pop_back().unwrap();
            if top == tmelcrypt::HashVal::default() {
                continue;
            }
            marked.set(top.as_ref());
            log::trace!(
                "gc: marking {}, queue size {}",
                hex::encode(top),
                queue.len()
            );
            let nd = smt::DBNode::from_bytes(
                &raw::Decoder::new()
                    .decompress_vec(txn.get(db, &top).expect("LMDB error"))
                    .unwrap(),
            );
            for p in nd.out_ptrs() {
                if p != Default::default() && enqueued.insert(p) {
                    queue.push_back(p);
                }
            }
            mark_ctr += 1;
        }
        // SWEEP phase
        {
            let mut cursor = txn.open_rw_cursor(db).unwrap();
            for (k, _) in cursor.iter_start() {
                if !marked.check(k) && k.len() == 32 {
                    log::trace!("gc: sweeping {}", hex::encode(k));
                    cursor.del(lmdb::WriteFlags::empty()).unwrap();
                    sweep_ctr += 1;
                }
            }
        }
        //txn.put(db, b"gc_roots", , flags)
        log::debug!(
            "gc: marked {}, swept {}, elapsed {:?}",
            mark_ctr,
            sweep_ctr,
            Instant::now() - start
        );
        txn.put(
            db,
            b"gc_mark",
            &((mark_ctr * 3 / 2) as u32).to_be_bytes(),
            lmdb::WriteFlags::empty(),
        )
        .unwrap();
        txn.commit().unwrap();
    }
}

fn gc_roots_helper(txn: &impl lmdb::Transaction, db: lmdb::Database) -> Vec<tmelcrypt::HashVal> {
    let out = txn.get(db, b"gc_roots").unwrap_or(b"");
    let mut toret = Vec::new();
    for i in 0..out.len() / 32 {
        toret.push(tmelcrypt::HashVal(
            (&out[i * 32..][..32]).try_into().unwrap(),
        ));
    }
    toret
}
