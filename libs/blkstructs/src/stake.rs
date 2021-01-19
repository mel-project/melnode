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
    use crate::{melscript, COINTYPE_TMEL, CoinID, CoinDataHeight, CoinData};
    use crate::State;
    use tmelcrypt::Ed25519SK;
    use std::collections::HashMap;

    /// Create a state from a staker mapping from sk to syms staking token for an epoch
    fn create_state(stakers: &HashMap<Ed25519SK, u64>, epoch_start: u64) -> State {
        // Create emtpy state
        let db = autosmt::DBManager::load(autosmt::MemDB::default());
        let mut state = State::new_empty(db);

        // Insert a mel coin into state so we can transact
        let start_micromels = 10000 as u64;
        let start_conshash = melscript::Script::always_true().hash();
        state.coins.insert(
            CoinID {
                txhash: tmelcrypt::HashVal([0; 32]),
                index: 0,
            },
            CoinDataHeight {
                coin_data: CoinData {
                    conshash: start_conshash,
                    value: start_micromels,
                    cointype: COINTYPE_TMEL.to_vec(),
                },
                height: 0,
            },
        );

        // Insert data need for staking proofs
        for (i, (sk, mets_staked)) in stakers.iter().enumerate() {
            state.stakes.insert(
                tmelcrypt::hash_single(&(i as u64).to_be_bytes()),
                StakeDoc {
                    pubkey: sk.to_public(),
                    e_start: epoch_start,
                    e_post_end: 1000000000,
                    mets_staked: *mets_staked,
                },
            );
        }
        state
    }

    #[test]
    fn test_non_staker_has_no_vote_power() {
        // Generate genesis block for stakers
        let staked_syms =vec![100 as u64; 3];
        let stakers = staked_syms.into_iter().map(|e| (tmelcrypt::ed25519_keygen().1, e)).collect();
        let genesis = create_state(&stakers, 0);

        // call vote_power for a key pair who is not a staker
        let (pk, _sk) = tmelcrypt::ed25519_keygen();
        let vote_power = genesis.stakes.vote_power(0, pk);

        // assert they have no vote power
        assert_eq!(vote_power, 0 as f64)
    }

    #[test]
    fn test_staker_has_correct_vote_power_in_epoch() {
        // Generate state for stakers
        let staked_syms =vec![100 as u64, 200 as u64, 300 as u64];
        let total_staked_syms: u64 = staked_syms.iter().sum();
        let stakers = staked_syms.into_iter().map(|e| (tmelcrypt::ed25519_keygen().1, e)).collect();
        let state = create_state(&stakers, 0);

        // Check the vote power of each staker in epoch 0 has expected value
        for (sk, vote) in stakers.iter() {
            let vote_power = state.stakes.vote_power(0, sk.to_public());
            let expected_vote_power = (*vote as f64) / (total_staked_syms as f64);
            assert_eq!(expected_vote_power - vote_power, 0.0 as f64);
        }
    }

    #[test]
    fn test_staker_has_no_vote_power_in_previous_epoch() {
        // Generate state for stakers
        let staked_syms =vec![100 as u64; 3];
        let stakers = staked_syms.into_iter().map(|e| (tmelcrypt::ed25519_keygen().1, e)).collect();
        let state = create_state(&stakers, 1);

        // Check the vote power of each staker in epoch 0 has expected value
        for (sk, _vote) in stakers.iter() {
            let vote_power = state.stakes.vote_power(0, sk.to_public());
            let expected_vote_power = 0.0 as f64;
            assert_eq!(vote_power, expected_vote_power);
        }
    }

    // #[test]
    // fn test_vote_power_single_staker_is_total() {
    //     let staker_key_pairs: Vec<(Ed25519PK, Ed25519SK)> = vec![
    //         tmelcrypt::ed25519_keygen(),
    //         tmelcrypt::ed25519_keygen(),
    //         tmelcrypt::ed25519_keygen(),
    //     ];
    //     let sk_stakers: Vec<Ed25519SK> = staker_key_pairs.iter().map(|e| e.1).collect();
    //
    //     let genesis_state = State::test_genesis(autosmt::DBManager::load(autosmt::MemDB::default()), 10000, melscript::Script::always_true().hash(), sk_stakers
    //         .iter()
    //         .map(|v| v.to_public())
    //         .collect::<Vec<_>>()
    //         .as_slice(),);
    //
    //     let stakes = genesis_state.stakes.clone();
    //
    //     let (pk, sk) = tmelcrypt::ed25519_keygen();
    //     let vote_power = stakes.vote_power(0, pk);
    //
    //     assert_eq!(vote_power, 0 as f64)
    // }
    //
    // #[test]
    // fn test_vote_power_no_stakers() {
    //
    // }
    //
    // #[test]
    // fn test_remove_stale() {
    //
    // }
    //
    // #[test]
    // fn test_keep_non_stale() {
    //
    // }
    //
    // #[test]
    // fn test_remove_stale_multiple_stakers() {
    //
    // }
}