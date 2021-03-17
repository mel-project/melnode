pub struct WalletStorage {
    wallets: SledMap<String, WalletData>
}

impl WalletStorage {
    /// Opens a WalletStorage, given a sled database.
    pub fn new(db: sled::Db) -> Self {
        let wallets = SledMap::new(db.open_tree("wallet").unwrap());
        Self {
            wallets
        }
    }
}
