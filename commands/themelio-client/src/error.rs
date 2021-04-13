use thiserror::Error;

#[derive(Error, Debug)]
/// A error that happens on the client side.
pub enum ClientError {
    #[error("invalid wallet name {:?}", .0)]
    InvalidWalletName(String),
    #[error("wallet with name {:?} already exists", .0)]
    DuplicateWalletName(String),
    #[error("provided secret does not unlock wallet with name {:?} ", .0)]
    InvalidWalletSecret(String),
    #[error("provided invalid input arguments to client {:?} ", .0)]
    InvalidInputArgs(String),
}