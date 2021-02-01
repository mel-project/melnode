use once_cell::sync::Lazy;
use tmelcrypt::HashVal;

/// Mel cointype
pub const DENOM_TMEL: &[u8] = b"m";

/// Sym cointype
pub const DENOM_TSYM: &[u8] = b"s";

/// DOSC cointype
pub const DENOM_DOSC: &[u8] = b"d";

/// Maximum coin value
pub const MAX_COINVAL: u128 = 1 << 120;

/// Auction interval
pub const AUCTION_INTERVAL: u64 = 20;

/// 1e6
pub const MICRO_CONVERTER: u128 = 1_000_000;

/// Entropy gathering block count
pub const ENTROPY_BLOCKS: usize = 1021;

/// Coin destruction covhash
pub const COVHASH_DESTROY: HashVal = HashVal([0; 32]);

/// ABID script covhash
pub static COVHASH_ABID: Lazy<HashVal> = Lazy::new(|| tmelcrypt::hash_keyed(b"special", b"ABID"));
