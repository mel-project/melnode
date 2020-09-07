use crate::SmtMapping;
use rlp_derive::*;

/// A stake epoch is 500,000 blocks.
pub const STAKE_EPOCH: u64 = 500_000;

/// StakeDoc is a stake document.
#[derive(RlpDecodable, RlpEncodable, Debug)]
pub struct StakeDoc {
    /// Public key.
    pub pubkey: tmelcrypt::Ed25519PK,
    /// Starting epoch.
    pub e_start: u64,
    /// Ending epoch. This is the epoch *after* the last epoch in which the mets are effective.
    pub e_post_end: u64,
    /// Number of mets staked.
    pub mets_staked: u64,
}

impl SmtMapping<tmelcrypt::HashVal, StakeDoc> {
    /// Gets the voting power, as a floating-point number, for a given public key and a given epoch.
    pub fn vote_power(&self, epoch: u64, pubkey: tmelcrypt::Ed25519PK) -> f64 {
        let mut total_votes = 1e-50;
        let mut target_votes = 0.0;
        for sdoc in self.val_iter() {
            if epoch >= sdoc.e_start && epoch < sdoc.e_post_end {
                total_votes += sdoc.mets_staked as f64;
                if sdoc.pubkey == pubkey {
                    target_votes += sdoc.mets_staked as f64;
                }
            }
        }
        target_votes / total_votes
    }

    /// Filter out all the elements that no longer matter.
    pub fn remove_stale(&mut self, epoch: u64) {
        let stale_key_hashes = self.mapping.iter().filter_map(|(kh, v)| {
            let v: StakeDoc = rlp::decode(&v).unwrap();
            if epoch > v.e_post_end {
                Some(kh)
            } else {
                None
            }
        });
        let mut new_tree = self.mapping.clone();
        for stale_key in stale_key_hashes {
            new_tree = new_tree.set(stale_key, b"");
        }
        self.mapping = new_tree
    }
}
