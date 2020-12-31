/// Mel cointype
pub const COINTYPE_TMEL: &[u8] = b"m";

/// Sym cointype
pub const COINTYPE_TSYM: &[u8] = b"s";

/// DOSC cointype
pub fn cointype_dosc(bn: u64) -> Vec<u8> {
    let week = bn / 20000;
    format!("d-{}", week).as_bytes().to_vec()
}

/// Maximum coin value
pub const MAX_COINVAL: u64 = 1 << 56;

/// Auction interval
pub const AUCTION_INTERVAL: u64 = 20;

/// 1e6
pub const MICRO_CONVERTER: u64 = 1_000_000;
