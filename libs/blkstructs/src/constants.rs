use tmelcrypt::HashVal;

/// Maximum coin value
pub const MAX_COINVAL: u128 = 1 << 120;

/// 1e6
pub const MICRO_CONVERTER: u128 = 1_000_000;

/// Coin destruction covhash
pub const COVHASH_DESTROY: HashVal = HashVal([0; 32]);
