use novasmt::{BackendNode, Hashed};

/// A sled-backed `autosmt` database.
pub struct SledTreeDB {
    disk_tree: sled::Tree,
}

impl SledTreeDB {
    /// Creates a new SledTreeDB based on a tree.
    pub fn new(disk_tree: sled::Tree) -> Self {
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

impl novasmt::BackendDB for SledTreeDB {
    fn get(&self, key: Hashed) -> Option<BackendNode> {
        let (bnode, _): BackendNodeRc =
            stdcode::deserialize(&self.disk_tree.get(&key).unwrap()?).unwrap();
        Some(bnode)
    }

    fn set_batch(&self, kvv: &[(Hashed, BackendNode)]) {
        self.disk_tree
            .transaction::<_, _, ()>(|tree| {
                let mut increment = Vec::new();
                log::debug!("inserting {} pairs", kvv.len());
                // first insert all the new elements with refcount 0, while keeping track of what to increment
                for (k, v) in kvv {
                    if let BackendNode::Internal(left, right) = v {
                        increment.push(*left);
                        increment.push(*right);
                    }
                    tree.insert(k.as_ref(), stdcode::serialize(&(v.clone(), 0)).unwrap())?;
                    // also delete from "delete tomorrow"
                    tree.remove(to_delete_tmrw(*k).as_ref())?;
                }
                // go through increment
                for increment in increment {
                    if increment != [0; 32] {
                        let mut bnrc: BackendNodeRc =
                            stdcode::deserialize(&tree.get(&increment)?.unwrap()).unwrap();
                        bnrc.1 += 1;
                        tree.insert(increment.as_ref(), stdcode::serialize(&bnrc).unwrap())?;
                    }
                }
                Ok(())
            })
            .unwrap();
    }

    fn delete_root(&self, key: Hashed) {
        self.disk_tree
            .transaction::<_, _, ()>(|tree| {
                tree.remove(to_delete_tmrw(key).as_ref())?;
                // DFS
                let mut dfs_stack: Vec<Hashed> = vec![key];
                while let Some(top) = dfs_stack.pop() {
                    if top != [0; 32] {
                        let (bnode, mut rcount): BackendNodeRc =
                            stdcode::deserialize(&tree.get(&top)?.unwrap()).unwrap();
                        rcount = rcount.saturating_sub(1);
                        if rcount == 0 {
                            log::debug!("deleting {}", hex::encode(&top));
                            tree.remove(&top);
                            if let BackendNode::Internal(left, right) = bnode {
                                dfs_stack.push(left);
                                dfs_stack.push(right);
                            }
                        }
                    }
                }
                Ok(())
            })
            .unwrap();
    }

    fn delete_root_tomorrow(&self, key: Hashed) {
        self.disk_tree
            .insert(to_delete_tmrw(key), b"dummy")
            .unwrap();
    }
}
