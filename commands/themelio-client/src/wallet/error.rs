use thiserror::Error;

#[derive(Error, Debug)]
/// An error that happens on the client side.
pub enum WalletError {
    #[error("invalid wallet name {:?}", .0)]
    InvalidName(String),
    #[error("wallet with name {:?} already exists", .0)]
    DuplicateName(String),
    #[error("provided secret does not unlock wallet with name {:?} ", .0)]
    InvalidSecret(String),
    #[error("Wallet with name {:?} not found", .0)]
    NotFound(String),
    #[error("provided invalid input arguments to client {:?} ", .0)]
    InvalidInputArgs(String),
}
