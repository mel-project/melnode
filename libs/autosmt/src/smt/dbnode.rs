use crate::smt::*;
use parking_lot::RwLock;
use std::convert::TryInto;
use std::sync::Arc;

// Internal nodes have 16 children and are identified by their 16-ary hash. Each child is 4 levels closer to the bottom.
// Finger nodes represent subtrees that only have one element. They include a bitvec representing remaining steps and the value itself.
#[derive(Clone)]
pub enum DBNode {
    Internal(InternalNode),
    Data(DataNode),
    Zero,
}

impl DBNode {
    /// Returns a vector of hash values representing outgoing pointers.
    pub fn out_ptrs(&self) -> Vec<tmelcrypt::HashVal> {
        unimplemented!()
    }
}

fn path_to_idx(path: &[bool]) -> usize {
    let path = &path[..4];
    let mut idx = 0;
    for &p in path {
        if p {
            idx += 1;
        }
        idx <<= 1;
    }
    idx >> 1
}

// impl DBNode {
//     pub fn hash(&self) -> [u8; 32] {
//         match self {
//             DBNode::Internal(int) => int.my_hash,
//             DBNode::Data(data) => data.hashes[0],
//         }
//     }

//     pub fn set_by_path(&self, key: [u8; 32], path: &[bool], data: &[u8]) -> DBNode {
//         match self {
//             DBNode::Data(dnode) => {
//                 if dnode.key == key {
//                     let mut newself = dnode.clone();
//                     newself.data = data.to_vec();
//                     newself.hashes = Vec::with_capacity(dnode.level);
//                     // let mut newself = match dnode {
//                     //     DataNode {
//                     //         db,
//                     //         key,
//                     //         data: _,
//                     //         level,
//                     //         hashes: _,
//                     //     } => DataNode {
//                     //         db: db.clone(),
//                     //         key: *key,
//                     //         data: data.to_vec(),
//                     //         level: *level,
//                     //         hashes: Vec::with_capacity(*level),
//                     //     },
//                     // };
//                     newself.write();
//                     return DBNode::Data(newself);
//                 }
//                 let empty = InternalNode::new_from_hash(dnode.db.clone(), dnode.level, [0; 32]);
//                 let old_key_idx =
//                     path_to_idx(&merk::key_to_path(dnode.key)[(256 - dnode.level as usize)..]);
//                 let mut newself = dnode.clone();
//                 newself.level -= 4;
//                 newself.hashes = Vec::new();
//                 newself.write();
//                 let newself = DBNode::Data(newself);
//                 let new = empty.set_gggc(old_key_idx, &newself);
//                 //drop(newself);
//                 // println!(
//                 //     "new is {}, newself = {}",
//                 //     hex::encode(new.my_hash),
//                 //     hex::encode(newself.hash()),
//                 // );
//                 DBNode::Internal(new).set_by_path(key, path, data)
//             }
//             DBNode::Internal(intnode) => {
//                 if intnode.my_hash == [0; 32] {
//                     let mut new_data = DataNode {
//                         db: intnode.db.clone(),
//                         key,
//                         data: data.to_vec(),
//                         level: intnode.level,
//                         hashes: Vec::with_capacity(intnode.level),
//                     };
//                     new_data.write();
//                     DBNode::Data(new_data)
//                 } else {
//                     let idx = path_to_idx(path);
//                     let newgggc = intnode.get_gggc(idx).set_by_path(key, &path[4..], data);
//                     let newnode = intnode.set_gggc(idx, &newgggc);
//                     DBNode::Internal(newnode)
//                 }
//             }
//         }
//     }

//     pub fn get_by_path_rev(&self, path: &[bool]) -> (Vec<u8>, Vec<[u8; 32]>) {
//         let path = path;
//         // go down the tree
//         match self {
//             DBNode::Data(dat) => (dat.data.clone(), dat.proof_frag()),
//             DBNode::Internal(intnode) => {
//                 if intnode.my_hash == [0; 32] {
//                     (vec![], vec![[0; 32]; 256 - intnode.level as usize])
//                 } else {
//                     let (nextbind, mut nextvec) = intnode
//                         .get_gggc(path_to_idx(path))
//                         .get_by_path_rev(&path[4..]);
//                     nextvec.append(&mut intnode.proof_frag(path));
//                     (nextbind, nextvec)
//                 }
//             }
//         }
//     }
// }

// Hexary database node. Encoded as 0 || first GGGC || ... || 16th GGGC
#[derive(Clone)]
pub struct InternalNode {
    my_hash: [u8; 32],
    ch_hashes: [[u8; 32]; 2],
    gc_hashes: [[u8; 32]; 4],
    ggc_hashes: [[u8; 32]; 8],
    gggc_hashes: [[u8; 32]; 16],
    level: usize,
}

fn other(idx: usize) -> usize {
    if idx % 2 == 0 {
        idx + 1
    } else {
        idx - 1
    }
}

// impl InternalNode {
//     pub fn new_from_bytes(level: usize, bytes: &[u8], given_hash: Option<[u8; 32]>) -> Self {
//         assert_eq!(bytes[0], 0);
//         let bytes = &bytes[1..];
//         let mut gggc_hashes = [[0; 32]; 16];
//         for i in 0..16 {
//             gggc_hashes[i] = bytes[i * 32..i * 32 + 32].try_into().unwrap();
//         }
//         let mut node = InternalNode {
//             my_hash: if let Some(h) = given_hash { h } else { [0; 32] },
//             ch_hashes: [[0; 32]; 2],
//             gc_hashes: [[0; 32]; 4],
//             ggc_hashes: [[0; 32]; 8],
//             gggc_hashes,
//             level,
//         };
//         node.write();
//         node
//     }

//     pub fn proof_frag(&self, path: &[bool]) -> Vec<[u8; 32]> {
//         let idx = path_to_idx(path);
//         let mut vec: Vec<[u8; 32]> = Vec::new();
//         vec.push(self.ch_hashes[other(idx / 8)]);
//         vec.push(self.gc_hashes[other(idx / 4)]);
//         vec.push(self.ggc_hashes[other(idx / 2)]);
//         vec.push(self.gggc_hashes[other(idx)]);
//         vec.reverse();
//         vec
//     }

//     pub fn get_gggc(&self, idx: usize) -> DBNode {
//         let bts = self.db.read().read(self.gggc_hashes[idx]).unwrap();
//         //println!("get_gggc read {}", hex::encode(&bts));
//         if bts[0] == 0 {
//             DBNode::Internal(InternalNode::new_from_bytes(
//                 self.db.clone(),
//                 self.level - 4,
//                 &bts,
//                 Some(self.gggc_hashes[idx]),
//             ))
//         } else {
//             let dat = DataNode::new_from_bytes(
//                 self.db.clone(),
//                 self.level - 4,
//                 &bts,
//                 Some(self.gggc_hashes[idx]),
//             );
//             DBNode::Data(dat)
//         }
//     }

//     pub fn set_gggc(&self, idx: usize, gggc: &DBNode<T>) -> Self {
//         let db = self.db.read();
//         let ghash = gggc.hash();
//         let mut newgg = self.gggc_hashes;
//         newgg[idx] = ghash;
//         let mut newnode = InternalNode {
//             level: self.level,
//             db: self.db.clone(),
//             ch_hashes: self.ch_hashes,
//             gc_hashes: self.gc_hashes,
//             ggc_hashes: self.ggc_hashes,
//             gggc_hashes: newgg,
//             my_hash: self.my_hash,
//         };
//         newnode.fix_hashes(idx);
//         drop(db);
//         newnode.write();
//         newnode
//     }

//     fn write(&mut self) {
//         self.cache_hashes();
//         let mut dbm = self.db.write();
//         if dbm.write(self.my_hash, &self.to_bytes()).is_none() {
//             dbm.refc_incr(self.my_hash).unwrap();
//         } else {
//             for h in self.gggc_hashes.iter() {
//                 if *h != [0; 32] {
//                     // println!("increasing refcount of {}", hex::encode(h));
//                     dbm.refc_incr(*h).unwrap();
//                 }
//             }
//         }
//     }

//     fn cache_hashes(&mut self) {
//         if self.my_hash == [0; 32] {
//             for i in 0..8 {
//                 self.ggc_hashes[i] =
//                     hash::node(self.gggc_hashes[i * 2], self.gggc_hashes[i * 2 + 1])
//             }
//             for i in 0..4 {
//                 self.gc_hashes[i] = hash::node(self.ggc_hashes[i * 2], self.ggc_hashes[i * 2 + 1])
//             }
//             for i in 0..2 {
//                 self.ch_hashes[i] = hash::node(self.gc_hashes[i * 2], self.gc_hashes[i * 2 + 1])
//             }
//             self.my_hash = hash::node(self.ch_hashes[0], self.ch_hashes[1])
//         }
//     }
//     fn fix_hashes(&mut self, _: usize) {
//         self.my_hash = [0; 32];
//         self.cache_hashes();
//         // let ggci = changed_idx / 2;
//         // self.ggc_hashes[ggci] =
//         //     hash::node(self.gggc_hashes[ggci * 2], self.gggc_hashes[ggci * 2 + 1]);
//         // let gci = ggci / 2;
//         // self.gc_hashes[gci] = hash::node(self.ggc_hashes[gci * 2], self.ggc_hashes[gci * 2 + 1]);
//         // let ci = gci / 2;
//         // self.ch_hashes[ci] = hash::node(self.gc_hashes[ci * 2], self.gc_hashes[ci * 2 + 1]);
//         // self.my_hash = hash::node(self.ch_hashes[0], self.ch_hashes[1]);
//     }
//     fn to_bytes(&self) -> Vec<u8> {
//         let mut vec = Vec::with_capacity(32 * 16 + 1);
//         vec.push(0);
//         for h in self.gggc_hashes.iter() {
//             vec.append(&mut h.to_vec());
//         }
//         vec
//     }
// }

// impl<T: database::Database> Drop for InternalNode<T> {
//     fn drop(&mut self) {
//         self.db.write().refc_decr_hex_recursive(self.my_hash);
//     }
// }

/// Subtree with only one element. Encoded as 1 || key || value
#[derive(Clone)]
pub struct DataNode {
    key: [u8; 32],
    data: Vec<u8>,
    level: usize,
    hashes: Vec<[u8; 32]>,
}

// impl<T: database::Database> DataNode<T> {
//     // pub fn new_from_hash(db: Rc<RefCell<T>>, level: u8, hash: [u8; 32]) -> Self {
//     //     let bytes = db.borrow().read(hash).unwrap();
//     //     DataNode::new_from_bytes(db, level, &bytes)
//     // }
//     fn new_from_bytes(
//         db: Arc<RwLock<T>>,
//         level: usize,
//         bytes: &[u8],
//         temp_hash: Option<[u8; 32]>,
//     ) -> Self {
//         assert_eq!(bytes[0], 1);
//         let bytes = &bytes[1..];
//         let mut node = DataNode {
//             db,
//             key: bytes[..32].try_into().unwrap(),
//             data: bytes[32..].to_vec(),
//             level,
//             hashes: if let Some(x) = temp_hash {
//                 vec![x]
//             } else {
//                 Vec::with_capacity(level)
//             },
//         };
//         node.write();
//         node
//     }
//     fn comp_hash(&mut self) {
//         if self.hashes.is_empty() {
//             let path = merk::key_to_path(self.key);
//             let path = &path[256 - self.level as usize..];
//             //assert_eq!(path.len(), self.level as usize);
//             let mut ptr = hash::datablock(&self.data);
//             self.hashes.push(ptr);
//             for data_on_right in path.iter().rev() {
//                 if *data_on_right {
//                     // add the opposite hash
//                     ptr = hash::node([0; 32], ptr);
//                 } else {
//                     ptr = hash::node(ptr, [0; 32]);
//                 }
//                 self.hashes.push(ptr)
//             }
//             self.hashes.reverse();
//         }
//     }

//     fn write(&mut self) {
//         self.comp_hash();
//         let mut dbm = self.db.write();
//         let hash = self.hashes[0];
//         // println!(
//         //     "writing out data with hash={}, level={}",
//         //     hex::encode(hash),
//         //     self.level
//         // );
//         let mut val = Vec::with_capacity(self.data.len() + 1);
//         val.push(1);
//         val.append(&mut self.key.to_vec());
//         val.append(&mut self.data.clone());
//         if dbm.write(hash, &val).is_none() {
//             dbm.refc_incr(hash).unwrap();
//         }
//     }

//     fn proof_frag(&self) -> Vec<[u8; 32]> {
//         vec![[0; 32]; self.level as usize]
//     }
// }
