use crate::SmtMapping;
use serde::{Deserialize, Serialize};

/// A stake epoch is 500,000 blocks.
pub const STAKE_EPOCH: u64 = 500_000;

/// StakeDoc is a stake document. It encapsulates all the information needed to verify consensus proofs.
#[derive(Serialize, Deserialize, Debug, Clone)]
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

/// A stake mapping
pub type StakeMapping = SmtMapping<tmelcrypt::HashVal, StakeDoc>;

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
            let v: StakeDoc = bincode::deserialize(&v).unwrap();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::melscript;
    use crate::State;
    use tmelcrypt::{Ed25519SK, Ed25519PK};
    use crate::TxKind::Stake;

    #[test]
    fn test_vote_power_non_staker() {
        let staker_key_pairs: Vec<(Ed25519PK, Ed25519SK)> = vec![
            tmelcrypt::ed25519_keygen(),
            tmelcrypt::ed25519_keygen(),
            tmelcrypt::ed25519_keygen(),
        ];
        let sk_stakers: Vec<Ed25519SK> = staker_key_pairs.iter().map(|e| e.1).collect();

        let genesis_state = State::test_genesis(autosmt::DBManager::load(autosmt::MemDB::default()), 10000, melscript::Script::always_true().hash(), sk_stakers
            .iter()
            .map(|v| v.to_public())
            .collect::<Vec<_>>()
            .as_slice(),);

        let stakes = genesis_state.stakes.clone();

        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let vote_power = stakes.vote_power(0, pk);

        assert_eq!(vote_power, 0 as f64)
    }

    #[test]
    fn test_vote_power_staker_not_in_epoch() {
        let staker_key_pairs: Vec<(Ed25519PK, Ed25519SK)> = vec![
            tmelcrypt::ed25519_keygen(),
            tmelcrypt::ed25519_keygen(),
            tmelcrypt::ed25519_keygen(),
        ];
        let sk_stakers: Vec<Ed25519SK> = staker_key_pairs.iter().map(|e| e.1).collect();

        let genesis_state = State::test_genesis(autosmt::DBManager::load(autosmt::MemDB::default()), 10000, melscript::Script::always_true().hash(), sk_stakers
            .iter()
            .map(|v| v.to_public())
            .collect::<Vec<_>>()
            .as_slice(),);

        let stakes = genesis_state.stakes.clone();

        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let vote_power = stakes.vote_power(0, pk);

        assert_eq!(vote_power, 0 as f64)
    }

    #[test]
    fn test_vote_power_staker() {
        let staker_key_pairs: Vec<(Ed25519PK, Ed25519SK)> = vec![
            tmelcrypt::ed25519_keygen(),
            tmelcrypt::ed25519_keygen(),
            tmelcrypt::ed25519_keygen(),
        ];
        let sk_stakers: Vec<Ed25519SK> = staker_key_pairs.iter().map(|e| e.1).collect();

        let genesis_state = State::test_genesis(autosmt::DBManager::load(autosmt::MemDB::default()), 10000, melscript::Script::always_true().hash(), sk_stakers
            .iter()
            .map(|v| v.to_public())
            .collect::<Vec<_>>()
            .as_slice(),);

        let stakes = genesis_state.stakes.clone();

        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let vote_power = stakes.vote_power(0, pk);

        assert_eq!(vote_power, 0 as f64)
    }

    #[test]
    fn test_vote_power_single_staker() {
        let staker_key_pairs: Vec<(Ed25519PK, Ed25519SK)> = vec![
            tmelcrypt::ed25519_keygen(),
            tmelcrypt::ed25519_keygen(),
            tmelcrypt::ed25519_keygen(),
        ];
        let sk_stakers: Vec<Ed25519SK> = staker_key_pairs.iter().map(|e| e.1).collect();

        let genesis_state = State::test_genesis(autosmt::DBManager::load(autosmt::MemDB::default()), 10000, melscript::Script::always_true().hash(), sk_stakers
            .iter()
            .map(|v| v.to_public())
            .collect::<Vec<_>>()
            .as_slice(),);

        let stakes = genesis_state.stakes.clone();

        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let vote_power = stakes.vote_power(0, pk);

        assert_eq!(vote_power, 0 as f64)
    }

    #[test]
    fn test_vote_power_multiple_stakers() {
        let staker_key_pairs: Vec<(Ed25519PK, Ed25519SK)> = vec![
            tmelcrypt::ed25519_keygen(),
            tmelcrypt::ed25519_keygen(),
            tmelcrypt::ed25519_keygen(),
        ];
        let sk_stakers: Vec<Ed25519SK> = staker_key_pairs.iter().map(|e| e.1).collect();

        let genesis_state = State::test_genesis(autosmt::DBManager::load(autosmt::MemDB::default()), 10000, melscript::Script::always_true().hash(), sk_stakers
            .iter()
            .map(|v| v.to_public())
            .collect::<Vec<_>>()
            .as_slice(),);

        let stakes = genesis_state.stakes.clone();

        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let vote_power = stakes.vote_power(0, pk);

        assert_eq!(vote_power, 0 as f64)
    }

    #[test]
    fn test_vote_power_no_stakers() {

    }

    #[test]
    fn test_remove_stale() {

    }

    #[test]
    fn test_keep_non_stale() {

    }

    #[test]
    fn test_remove_stale_multiple_stakers() {

    }
}