use tmelcrypt::HashVal;

/// Mel cointype
pub const DENOM_TMEL: &[u8] = b"m";

/// Sym cointype
pub const DENOM_TSYM: &[u8] = b"s";

/// DOSC cointype
pub const DENOM_DOSC: &[u8] = b"d";

/// New cointype
pub const DENOM_NEWCOIN: &[u8] = b"";

/// Maximum coin value
pub const MAX_COINVAL: u128 = 1 << 120;

/// 1e6
pub const MICRO_CONVERTER: u128 = 1_000_000;

/// Coin destruction covhash
pub const COVHASH_DESTROY: HashVal = HashVal([0; 32]);
