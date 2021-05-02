mod cstate;
mod msg;
mod protocol;
pub use protocol::*;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
