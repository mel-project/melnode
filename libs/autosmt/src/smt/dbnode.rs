use crate::smt::*;
use enum_dispatch::enum_dispatch;
use std::convert::TryInto;
use tmelcrypt::HashVal;

// Internal nodes have 16 children and are identified by their 16-ary hash. Each child is 4 levels closer to the bottom.
// Data nodes represent subtrees that only have one element. They include a bitvec representing remaining steps and the value itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DBNode {
    Internal(InternalNode),
    Data(DataNode),
    Zero,
}
use DBNode::*;

#[enum_dispatch]
pub(crate) trait DBNodeT {
    fn out_ptrs(&self) -> Vec<tmelcrypt::HashVal>;
    fn from_bytes(bts: &[u8]) -> Self;
    fn hash(&self) -> tmelcrypt::HashVal;
}

impl DBNode {
    /// Returns a vector of hash values representing outgoing pointers.
    pub fn out_ptrs(&self) -> Vec<tmelcrypt::HashVal> {
        match self {
            Internal(int) => int.gggc_hashes.to_vec(),
            _ => vec![],
        }
    }

    /// Returns a vector of hash values representing outgoing non-null pointers.
    pub fn out_ptrs_nonnull(&self) -> Vec<tmelcrypt::HashVal> {
        match self {
            Internal(int) => int
                .gggc_hashes
                .iter()
                .filter(|v| **v != HashVal::default())
                .cloned()
                .collect(),
            _ => vec![],
        }
    }

    /// From bytes.
    pub fn from_bytes(bts: &[u8]) -> Self {
        match bts[0] {
            0 => Internal(InternalNode::from_bytes(bts)),
            1 => Data(DataNode::from_bytes(bts)),
            x => panic!("invalid DBNode type {}", x),
        }
    }

    /// To bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        // assert_eq!(self, &DBNode::from_bytes(&out));
        match self {
            Internal(int) => int.to_bytes(),
            Data(dat) => dat.to_bytes(),
            Zero => vec![],
        }
    }

    /// Root-hash.
    pub fn hash(&self) -> tmelcrypt::HashVal {
        match self {
            Internal(int) => int.my_hash,
            Data(dat) => dat.calc_hash(),
            Zero => tmelcrypt::HashVal::default(),
        }
    }

    /// Get by path rev
    pub fn get_by_path_rev(
        &self,
        path: &[bool],
        key: tmelcrypt::HashVal,
        db: &Forest,
    ) -> (Vec<u8>, Vec<tmelcrypt::HashVal>) {
        match self {
            Internal(int) => int.get_by_path_rev(path, key, db),
            Data(dat) => dat.get_by_path_rev(path, key, db),
            Zero => (
                vec![],
                path.iter().map(|_| tmelcrypt::HashVal::default()).collect(),
            ),
        }
    }

    pub fn set_by_path(
        &self,
        path: &[bool],
        key: tmelcrypt::HashVal,
        data: &[u8],
        db: &Forest,
    ) -> Self {
        match self {
            Internal(int) => int.set_by_path(path, key, data, db),
            Data(dat) => dat.set_by_path(path, key, data, db),
            Zero => {
                let d = Data(DataNode {
                    key,
                    level: path.len(),
                    data: data.to_vec(),
                });
                db.write(d.hash(), d.clone());
                d
            }
        }
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

// Hexary database node. Encoded as 0 || first GGGC || ... || 16th GGGC
#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct InternalNode {
    my_hash: tmelcrypt::HashVal,
    ch_hashes: [tmelcrypt::HashVal; 2],
    gc_hashes: [tmelcrypt::HashVal; 4],
    ggc_hashes: [tmelcrypt::HashVal; 8],
    gggc_hashes: [tmelcrypt::HashVal; 16],
}

fn other(idx: usize) -> usize {
    if idx % 2 == 0 {
        idx + 1
    } else {
        idx - 1
    }
}

impl InternalNode {
    fn from_bytes(bytes: &[u8]) -> Self {
        assert_eq!(bytes[0], 0);
        let bytes = &bytes[1..];
        let zero = tmelcrypt::HashVal::default();
        let mut gggc_hashes = [zero; 16];
        for i in 0..16 {
            gggc_hashes[i] = tmelcrypt::HashVal(bytes[i * 32..i * 32 + 32].try_into().unwrap());
        }
        let mut node = InternalNode {
            my_hash: zero,
            ch_hashes: [zero; 2],
            gc_hashes: [zero; 4],
            ggc_hashes: [zero; 8],
            gggc_hashes,
        };
        node.cache_hashes();
        node
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut toret = Vec::with_capacity(128);
        toret.push(0);
        for v in self.gggc_hashes.iter() {
            toret.extend_from_slice(&v.0);
        }
        toret
    }

    fn cache_hashes(&mut self) {
        assert_eq!(self.my_hash, tmelcrypt::HashVal::default());
        for i in 0..8 {
            self.ggc_hashes[i] = hash::node(self.gggc_hashes[i * 2], self.gggc_hashes[i * 2 + 1])
        }
        for i in 0..4 {
            self.gc_hashes[i] = hash::node(self.ggc_hashes[i * 2], self.ggc_hashes[i * 2 + 1])
        }
        for i in 0..2 {
            self.ch_hashes[i] = hash::node(self.gc_hashes[i * 2], self.gc_hashes[i * 2 + 1])
        }
        self.my_hash = hash::node(self.ch_hashes[0], self.ch_hashes[1])
    }

    fn fix_hashes(&mut self, idx: usize) {
        let ggci = idx / 2;
        self.ggc_hashes[ggci] =
            hash::node(self.gggc_hashes[ggci * 2], self.gggc_hashes[ggci * 2 + 1]);
        let gci = ggci / 2;
        self.gc_hashes[gci] = hash::node(self.ggc_hashes[gci * 2], self.ggc_hashes[gci * 2 + 1]);
        let ci = gci / 2;
        self.ch_hashes[ci] = hash::node(self.gc_hashes[ci * 2], self.gc_hashes[ci * 2 + 1]);
        self.my_hash = hash::node(self.ch_hashes[0], self.ch_hashes[1]);
    }

    fn get_by_path_rev(
        &self,
        path: &[bool],
        key: tmelcrypt::HashVal,
        db: &Forest,
    ) -> (Vec<u8>, Vec<tmelcrypt::HashVal>) {
        let (nextbind, mut nextvec) =
            self.get_gggc(path_to_idx(path), db)
                .get_by_path_rev(&path[4..], key, db);
        nextvec.extend_from_slice(&self.proof_frag(path));
        (nextbind, nextvec)
    }

    fn set_by_path(
        &self,
        path: &[bool],
        key: tmelcrypt::HashVal,
        data: &[u8],
        db: &Forest,
    ) -> DBNode {
        let idx = path_to_idx(path);
        let newgggc = self
            .get_gggc(idx, db)
            .set_by_path(&path[4..], key, data, db);
        let mut newself = self.clone();
        newself.gggc_hashes[idx] = newgggc.hash();
        newself.fix_hashes(idx);
        db.write(newself.my_hash, Internal(newself.clone()));
        Internal(newself)
    }

    fn proof_frag(&self, path: &[bool]) -> Vec<tmelcrypt::HashVal> {
        let idx = path_to_idx(path);
        let mut vec = Vec::new();
        vec.push(self.ch_hashes[other(idx / 8)]);
        vec.push(self.gc_hashes[other(idx / 4)]);
        vec.push(self.ggc_hashes[other(idx / 2)]);
        vec.push(self.gggc_hashes[other(idx)]);
        vec.reverse();
        vec
    }

    fn get_gggc(&self, idx: usize, db: &Forest) -> DBNode {
        db.read(self.gggc_hashes[idx])
    }
}

/// Subtree with only one element. Encoded as 1 || level || key || value
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataNode {
    pub(crate) level: usize,
    pub(crate) key: tmelcrypt::HashVal,
    pub(crate) data: Vec<u8>,
}

impl DataNode {
    fn from_bytes(bts: &[u8]) -> Self {
        assert_eq!(bts[0], 1);
        let level = u16::from_be_bytes(bts[1..3].try_into().unwrap());
        let bytes = &bts[3..];
        DataNode {
            level: level as usize,
            key: tmelcrypt::HashVal(bytes[..32].try_into().unwrap()),
            data: bytes[32..].to_vec(),
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut toret = Vec::with_capacity(256);
        toret.push(1);
        toret.extend_from_slice(&(self.level as u16).to_be_bytes());
        toret.extend_from_slice(&self.key.0);
        toret.extend_from_slice(&self.data);
        toret
    }

    fn calc_hash(&self) -> tmelcrypt::HashVal {
        merk::data_hashes(self.key, &self.data)[256 - self.level as usize]
    }

    fn get_by_path_rev(
        &self,
        _: &[bool],
        key: tmelcrypt::HashVal,
        _: &Forest,
    ) -> (Vec<u8>, Vec<tmelcrypt::HashVal>) {
        (
            if self.key == key {
                self.data.clone()
            } else {
                vec![]
            },
            self.proof_frag(),
        )
    }

    fn set_by_path(
        &self,
        path: &[bool],
        key: tmelcrypt::HashVal,
        data: &[u8],
        db: &Forest,
    ) -> DBNode {
        if self.key == key {
            // eprintln!(
            //     "overwriting key {:?} with data {:?} (prev {:?})",
            //     key, data, self.data
            // );
            // if data.is_empty() {
            //     return Zero;
            // }
            let mut newself = self.clone();
            newself.data = data.to_vec();
            db.write(newself.calc_hash(), Data(newself.clone()));
            return Data(newself);
        }
        // general case: we move ourselves down 4 levels, set that as a grandchild of a new internal node, and insert the key into that internal node
        let mut newself = self.clone();
        // eprintln!(
        //     "moving {:?} from {} to {}",
        //     newself.key,
        //     newself.level,
        //     newself.level - 4
        // );
        newself.level -= 4;
        let newhash = newself.calc_hash();
        db.write(newhash, Data(newself));
        let mut newint = InternalNode::default();
        let old_key_idx = path_to_idx(&merk::key_to_path(self.key)[(256 - self.level as usize)..]);
        newint.gggc_hashes[old_key_idx] = newhash;
        newint.fix_hashes(old_key_idx);
        db.write(newint.my_hash, Internal(newint.clone()));
        Internal(newint).set_by_path(path, key, data, db)
    }

    fn proof_frag(&self) -> Vec<tmelcrypt::HashVal> {
        vec![tmelcrypt::HashVal::default(); self.level as usize]
    }
}
