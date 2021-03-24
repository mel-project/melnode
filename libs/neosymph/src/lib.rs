pub mod msg;
mod streamlet;
pub use streamlet::*;
mod protocol;
pub use msg::ConfirmResp;
pub use protocol::*;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
