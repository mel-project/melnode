/// Uses sled map(s) to persist client-side data
pub struct ClientStorage {
    wallets: SledMap<String, WalletData>
}

impl ClientStorage {
    pub fn new(db: sled::Db) -> Self {
        let wallets = SledMap::new(db.open_tree("wallet").unwrap());
        Self {
            wallets
        }
    }
}
