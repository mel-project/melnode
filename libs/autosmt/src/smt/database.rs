use parking_lot::{Mutex, RwLock};
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::collections::HashSet;
use std::convert::TryInto;
use std::sync::Arc;

/// Database represents a backend for storing SMT nodes.
pub trait Database: Send + Sync {
    /// Writes a hash-value mapping into the database with refcount=1, returning the hash. Fails if the value already exists.
    fn write(&mut self, key: [u8; 32], val: &[u8]) -> Option<()>;
    /// Increments a reference count for a hash, returning the new refcount. Fails if no such key.
    fn refc_incr(&mut self, key: [u8; 32]) -> Option<usize>;
    /// Decrements a reference count for a hash, returning the new refcount. If the refcount reaches zero, then delete the key-value binding. Fails if no such key.
    fn refc_decr(&mut self, key: [u8; 32]) -> Option<usize>;
    /// Recursively decrements reference counts, going down children for all 513-byte hexary nodes if the root dies.
    fn refc_decr_hex_recursive(&mut self, root_key: [u8; 32]) -> Option<usize> {
        if root_key == [0; 32] {
            return Some(0);
        }
        let root_data = self.read(root_key)?;
        match self.refc_decr(root_key)? {
            0 => {
                if root_data.len() == 513 && root_data[0] == 0 {
                    for child_i in 0..16 {
                        let child_hash = root_data[child_i * 32 + 1..][..32].try_into().unwrap();
                        self.refc_decr_hex_recursive(child_hash);
                    }
                }
                Some(0)
            }
            x => Some(x),
        }
    }
    /// Reads a mapping.
    fn read(&self, key: [u8; 32]) -> Option<Vec<u8>>;
}

pub fn wrap_db<T: Database>(db: T) -> Arc<RwLock<T>> {
    Arc::new(RwLock::new(db))
}

type TdbMapEntry = (Vec<u8>, RefCell<usize>);
pub struct TrivialDB {
    mapping: Mutex<HashMap<[u8; 32], TdbMapEntry>>,
}

impl Default for TrivialDB {
    fn default() -> Self {
        TrivialDB::new()
    }
}

impl TrivialDB {
    pub fn new() -> Self {
        TrivialDB {
            mapping: Mutex::new(HashMap::new()),
        }
    }

    pub fn count(&self) -> usize {
        self.mapping.lock().len()
    }

    pub fn graphviz(&self) -> String {
        let mut toret = String::new();
        toret.push_str("digraph G{\n");
        for (k, v) in self.mapping.lock().iter() {
            let hk = hex::encode(&k[0..5]);
            let label = format!("[label = \"{}-[{}]\"]", hk, v.1.borrow());
            toret.push_str(&format!("\"{}\" {}\n", hk, label));
            let vec = &v.0[1..];
            if vec.len() % 32 == 0 {
                for i in 0..vec.len() / 32 {
                    let ptr = hex::encode(&vec[i * 32..i * 32 + 5]);
                    if ptr != "0000000000" {
                        toret.push_str(&format!("\"{}\" -> \"{}\" [label=\"{}\"]\n", hk, ptr, i));
                    }
                }
            }
        }
        toret.push_str("}");
        toret
    }
}

impl Database for TrivialDB {
    fn write(&mut self, key: [u8; 32], val: &[u8]) -> Option<()> {
        if key == [0; 32] {
            return Some(());
        }
        let mut mapping = self.mapping.lock();
        if let Some((_, _)) = mapping.get(&key) {
            None
        } else {
            mapping.insert(key, (val.to_vec(), RefCell::new(1)));
            Some(())
        }
    }

    fn refc_incr(&mut self, key: [u8; 32]) -> Option<usize> {
        if key == [0; 32] {
            return Some(1);
        }
        let mapping = self.mapping.lock();
        if let Some((_, refcount)) = mapping.get(&key) {
            let mut mref = refcount.borrow_mut();
            *mref += 1;
            Some(*mref)
        } else {
            println!("can't incr {}", hex::encode(&key[0..10]));
            None
        }
    }

    fn refc_decr(&mut self, key: [u8; 32]) -> Option<usize> {
        if key == [0; 32] {
            return Some(1);
        }
        let mut mapping = self.mapping.lock();
        if let Entry::Occupied(mut occupied) = mapping.entry(key) {
            let result = {
                let ks = occupied.get_mut();
                let mut mref = ks.1.borrow_mut();
                *mref -= 1;
                *mref
            };
            if result == 0 {
                occupied.remove();
            }
            Some(result)
        } else {
            None
        }
    }

    fn read(&self, key: [u8; 32]) -> Option<Vec<u8>> {
        if key == [0; 32] {
            return Some([0; 32 * 16 + 1].to_vec());
        }
        let mapping = self.mapping.lock();
        let v = mapping.get(&key)?;
        Some(v.0.clone())
    }
}

/// PersistentDatabase represents a persistent database, and it's a supertrait for Database.
pub trait PersistentDatabase: Database {
    // Sets a persistent node at a certain index
    fn set_persist(&mut self, idx: usize, key: [u8; 32]);
    // Gets a persistent node at a certain index
    fn get_persist(&self, idx: usize) -> Option<[u8; 32]>;
    // Sync to disk, returning only when data is durable
    fn sync(&mut self);
}

/// RawKeyVal represents a raw key-value store, similar to that offered by key-value databases.
pub trait RawKeyVal: Send + Sync {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    fn set(&mut self, key: &[u8], val: Option<&[u8]>) {
        let mut vec = Vec::new();
        vec.push((
            key.to_vec(),
            match val {
                Some(x) => Some(x.to_vec()),
                None => None,
            },
        ));
        self.set_batch(vec);
    }
    fn set_batch<T>(&mut self, kvv: T)
    where
        T: IntoIterator<Item = (Vec<u8>, Option<Vec<u8>>)>;
}

/// CacheDatabase is a PersistentDatabase that wraps a RawKeyVal, providing caching functionality.
pub struct CacheDatabase<T: RawKeyVal> {
    diskdb: T,
    refc_cache: HashMap<[u8; 32], u64>,
    refc_orig: HashMap<[u8; 32], u64>,
    bind_cache: HashMap<[u8; 32], Vec<u8>>,
    persist_cache: HashMap<usize, [u8; 32]>,
    ephem: HashSet<[u8; 32]>,
}

impl<T: RawKeyVal> CacheDatabase<T> {
    pub fn new(db: T) -> Self {
        CacheDatabase {
            diskdb: db,
            refc_cache: HashMap::new(),
            bind_cache: HashMap::new(),
            persist_cache: HashMap::new(),
            ephem: HashSet::new(),
            refc_orig: HashMap::new(),
        }
    }

    fn refc_load(&mut self, key: [u8; 32]) -> Option<()> {
        if self.refc_cache.get(&key).is_some() {
            return Some(());
        }
        let ri = u64::from_le_bytes(
            self.diskdb
                .get(&to_refkey(&key))?
                .as_slice()
                .try_into()
                .unwrap(),
        );
        self.refc_cache.insert(key, ri);
        self.refc_orig.insert(key, ri);
        Some(())
    }
}

fn to_refkey(key: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(key.len() + 1);
    v.extend_from_slice(key);
    v.push(1);
    v
}
fn to_nodekey(key: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(key.len() + 1);
    v.extend_from_slice(key);
    v.push(0);
    v
}

impl<T: RawKeyVal> Database for CacheDatabase<T> {
    fn read(&self, key: [u8; 32]) -> Option<Vec<u8>> {
        if key == [0; 32] {
            return Some([0; 32 * 16 + 1].to_vec());
        }
        if let Some(vec) = self.bind_cache.get(&key) {
            Some(vec.clone())
        } else {
            let fresh_vec = self.diskdb.get(&to_nodekey(&key))?.to_vec();
            //bind_cache.insert(key, fresh_vec.clone());
            Some(fresh_vec)
        }
    }
    fn write(&mut self, key: [u8; 32], val: &[u8]) -> Option<()> {
        if self.read(key).is_some() {
            return None;
        }
        if self.bind_cache.insert(key, val.to_vec()).is_none() {
            self.refc_cache.insert(key, 1);
            self.ephem.insert(key);
        }
        if self.bind_cache.len() > 20000 {
            self.sync()
        }
        Some(())
    }

    fn refc_incr(&mut self, key: [u8; 32]) -> Option<usize> {
        if key == [0; 32] {
            return Some(1);
        }
        self.refc_load(key)?;
        let r = self.refc_cache.get_mut(&key).unwrap();
        *r += 1;
        Some(*r as usize)
    }
    fn refc_decr(&mut self, key: [u8; 32]) -> Option<usize> {
        if key == [0; 32] {
            return Some(1);
        }
        self.refc_load(key)?;
        let r = self.refc_cache.get_mut(&key).unwrap();
        *r -= 1;
        let r = *r;
        if r == 0 {
            if self.ephem.get(&key).is_some() {
                self.ephem.remove(&key);
                self.bind_cache.remove(&key);
                self.refc_cache.remove(&key);
            } else {
                self.refc_cache.insert(key, 0);
            }
        }
        Some(r as usize)
    }
}

impl<T: RawKeyVal> PersistentDatabase for CacheDatabase<T> {
    fn set_persist(&mut self, idx: usize, key: [u8; 32]) {
        let old = self.get_persist(idx);
        self.persist_cache.insert(idx, key);
        self.refc_incr(key);
        // recursively drop
        if let Some(old) = old {
            self.refc_decr_hex_recursive(old).unwrap();
        }
    }
    fn get_persist(&self, idx: usize) -> Option<[u8; 32]> {
        if let Some(v) = self.persist_cache.get(&idx) {
            Some(*v)
        } else {
            let idx = idx as u64;
            let fresh: [u8; 32] = self
                .diskdb
                .get(&idx.to_le_bytes().to_vec())?
                .as_slice()
                .try_into()
                .unwrap();
            Some(fresh)
        }
    }
    fn sync(&mut self) {
        // println!(
        //     "sync with {} in bind_cache, {} in refc_cache",
        //     self.bind_cache.len(),
        //     self.refc_cache.len()
        // );
        let mut to_batch: Vec<(Vec<u8>, Option<Vec<u8>>)> = Vec::new();
        // iterate through dirty keys
        for (k, v) in self.bind_cache.iter() {
            to_batch.push((to_nodekey(k), Some(v.clone())));
        }
        for (k, v) in self.refc_cache.iter() {
            let origv = *self.refc_orig.get(k).unwrap_or(&0);
            if *v != origv {
                //println!("new {}, old {}", *v, origv);
                if *v == 0 {
                    to_batch.push((to_nodekey(k), None));
                    to_batch.push((to_refkey(k), None));
                } else {
                    to_batch.push((to_refkey(k), Some((*v as u64).to_le_bytes().to_vec())));
                }
            }
        }
        for (k, v) in self.persist_cache.iter() {
            to_batch.push(((*k as u64).to_le_bytes().to_vec(), Some(v.to_vec())));
        }
        // commit
        self.diskdb.set_batch(to_batch);
        // clear ephemeral
        self.ephem.clear();
        self.refc_orig.clear();
        self.bind_cache.clear();
        self.refc_cache.clear();
    }
}

impl<T: RawKeyVal> Drop for CacheDatabase<T> {
    fn drop(&mut self) {
        self.sync()
    }
}
