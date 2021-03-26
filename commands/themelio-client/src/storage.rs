use sled;
use std::path::PathBuf;

/// Uses sled map(s) to persist client-side data
pub struct ClientStorage {
    // wallets: SledMap<String, WalletData>
    path: PathBuf,
    name: String
}

impl ClientStorage {
    pub fn new(db: sled::Db) -> Self {
        Self {}
        // let wallets = SledMap::new(db.open_tree("wallet").unwrap());
        // Self {
        //     wallets
        // }
    }

    pub fn insert(&self, key: &String, value: &String) {
        let db = sled::open(&self.path).unwrap();
        let tree = db.open_tree(b"client")
        NodeStorage::new(sled::open(&opt.database).unwrap(), testnet_genesis_config().await).share();
        sled.open()
        // Check if wallet with same name already exits
        if storage.get(&name);
    }
}
