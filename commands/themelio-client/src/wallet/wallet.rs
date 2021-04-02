use crate::wallet::data::WalletData;
use blkstructs::melvm::Covenant;

struct Wallet {
    name: String
}

impl Wallet {
    pub fn new(name: &str, covenant: Covenant) -> Self {
       let wallet_data = WalletData::new(covenant); 
    }
}