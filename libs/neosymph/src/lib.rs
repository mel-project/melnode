pub mod msg;
mod streamlet;
use blkstructs::ProposerAction;
pub use streamlet::*;
mod protocol;
pub use protocol::*;
use tmelcrypt::HashVal;

/// The only allowed ProposerAction for OOB blocks.
pub static OOB_PROPOSER_ACTION: ProposerAction = ProposerAction {
    fee_multiplier_delta: 0,
    reward_dest: HashVal([0; 32]),
};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
