use thiserror::Error;

#[derive(Error, Debug)]
/// A error that happens on the client side
pub enum ClientError {
    #[error("invalid wallet name {:?}", .0)]
    InvalidWalletName(String),
    #[error("wallet with name {:?} already exists", .0)]
    WalletDuplicate(String),
    #[error("provided secret does not unlock wallet with name {:?} ", .0)]
    InvalidWalletSecret(String),
    // #[error("attempted to spend non-existent coin {:?}", .0)]
    // NonexistentCoin(txn::CoinID),
    // #[error("unbalanced inputs and outputs")]
    // UnbalancedInOut,
    // #[error("insufficient fees (requires {0})")]
    // InsufficientFees(u128),
    // #[error("referenced non-existent script {:?}", .0)]
    // NonexistentScript(tmelcrypt::HashVal),
    // #[error("does not satisfy script {:?}", .0)]
    // ViolatesScript(tmelcrypt::HashVal),
    // #[error("invalid sequential proof of work")]
    // InvalidMelPoW,
    // #[error("auction bid at wrong time")]
    // BidWrongTime,
    // #[error("block has wrong header after applying to previous block")]
    // WrongHeader,
    // #[error("tried to spend locked coin")]
    // CoinLocked,
}