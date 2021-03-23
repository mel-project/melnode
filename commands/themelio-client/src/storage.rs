// pub use sled_tree::*;

// mod sled_map;
// pub use sled_map::*;

/// Uses sled map(s) to persist client-side data
pub struct ClientStorage {
    // wallets: SledMap<String, WalletData>
}

impl ClientStorage {
    pub fn new(db: sled::Db) -> Self {
        Self {}
        // let wallets = SledMap::new(db.open_tree("wallet").unwrap());
        // Self {
        //     wallets
        // }
    }
}
