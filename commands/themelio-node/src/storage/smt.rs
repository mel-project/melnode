use novasmt::{BackendNode, Hashed};

/// A boringdb-backed `autosmt` database.
pub struct BoringDbSmt {
    disk_tree: boringdb::Dict,
}

impl BoringDbSmt {
    /// Creates a new database based on a tree.
    pub fn new(disk_tree: boringdb::Dict) -> Self {
        Self { disk_tree }
    }
}

fn to_delete_tmrw(key: [u8; 32]) -> [u8; 33] {
    let mut toret = [0; 33];
    toret[0] = 0xff;
    toret[1..].copy_from_slice(&key);
    toret
}

type BackendNodeRc = (BackendNode, u64);

impl novasmt::BackendDB for BoringDbSmt {
    fn get(&self, key: Hashed) -> Option<BackendNode> {
        if let Some(val) = self.disk_tree.get(&key).unwrap() {
            let (bnode, _): BackendNodeRc = stdcode::deserialize(&val).unwrap();
            Some(bnode)
        } else {
            None
        }
    }

    fn set_batch(&self, kvv: &[(Hashed, BackendNode)]) {
        let mut tree = self.disk_tree.transaction().unwrap();
        let mut increment = Vec::new();
        log::trace!("inserting {} pairs", kvv.len());
        // first insert all the new elements with refcount 0, while keeping track of what to increment
        for (k, v) in kvv {
            if let BackendNode::Internal(left, right) = v {
                increment.push(*left);
                increment.push(*right);
            }
            tree.insert(k.to_vec(), stdcode::serialize(&(v.clone(), 0)).unwrap())
                .unwrap();
            // also delete from "delete tomorrow"
            tree.remove(to_delete_tmrw(*k).as_ref()).unwrap();
        }
        // go through increment
        for increment in increment {
            if increment != [0; 32] {
                let mut bnrc: BackendNodeRc =
                    stdcode::deserialize(&tree.get(&increment).unwrap().unwrap()).unwrap();
                bnrc.1 += 1;
                tree.insert(increment.to_vec(), stdcode::serialize(&bnrc).unwrap())
                    .unwrap();
            }
        }
    }

    fn delete_root(&self, key: Hashed) {
        let mut tree = self.disk_tree.transaction().unwrap();
        tree.remove(to_delete_tmrw(key).as_ref()).unwrap();
        // DFS
        let mut dfs_stack: Vec<Hashed> = vec![key];
        while let Some(top) = dfs_stack.pop() {
            if top != [0; 32] {
                let (bnode, mut rcount): BackendNodeRc =
                    stdcode::deserialize(&tree.get(&top).unwrap().unwrap()).unwrap();
                log::debug!("rcount {}", rcount);
                rcount = rcount.saturating_sub(1);
                if rcount == 0 {
                    log::debug!("deleting {}", hex::encode(&top));
                    tree.remove(&top).unwrap();
                    if let BackendNode::Internal(left, right) = bnode {
                        dfs_stack.push(left);
                        dfs_stack.push(right);
                    }
                }
            }
        }
    }

    fn delete_root_tomorrow(&self, key: Hashed) {
        self.disk_tree
            .insert(to_delete_tmrw(key).to_vec(), b"dummy".to_vec())
            .unwrap();
    }
}
