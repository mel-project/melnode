pub use crate::stake::*;
use crate::{constants::*, melvm::Covenant, preseal_melmint, CoinDataHeight, Denom};
use crate::{smtmapping::*, CoinData};
use crate::{transaction as txn, CoinID};
use applytx::StateHandle;
use defmac::defmac;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde_repr::{Deserialize_repr, Serialize_repr};

use arbitrary::Arbitrary;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::{collections::BTreeMap, convert::TryInto};
use std::{collections::BTreeSet, io::Read};
use thiserror::Error;
use tmelcrypt::{Ed25519PK, HashVal};
use txn::Transaction;

use self::melswap::PoolMapping;
mod applytx;
pub(crate) mod melmint;
pub(crate) mod melswap;

// TODO: Move these structs into state package
// ie: split this into modules such as
// error.rs header.rs seal.rs propser.rs block.rs state.rs and lib
// and put them into the state folder or rename state folder to blk folder

#[derive(Error, Debug)]
/// A error that happens while applying a transaction to a state
pub enum StateError {
    #[error("malformed transaction")]
    MalformedTx,
    #[error("attempted to spend non-existent coin {:?}", .0)]
    NonexistentCoin(txn::CoinID),
    #[error("unbalanced inputs and outputs")]
    UnbalancedInOut,
    #[error("insufficient fees (requires {0})")]
    InsufficientFees(u128),
    #[error("referenced non-existent script {:?}", .0)]
    NonexistentScript(tmelcrypt::HashVal),
    #[error("does not satisfy script {:?}", .0)]
    ViolatesScript(tmelcrypt::HashVal),
    #[error("invalid sequential proof of work")]
    InvalidMelPoW,
    #[error("auction bid at wrong time")]
    BidWrongTime,
    #[error("block has wrong header after applying to previous block")]
    WrongHeader,
    #[error("tried to spend locked coin")]
    CoinLocked,
    #[error("duplicate transaction")]
    DuplicateTx,
}

/// Configuration of a genesis state. Serializable via serde.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenesisConfig {
    /// What kind of network?
    pub network: NetID,
    /// Initial supply of free money. This will be put at the zero-zero coin ID.
    pub init_coindata: CoinData,
    /// Mapping of initial stakeholders.
    pub stakes: BTreeMap<HashVal, StakeDoc>,
    /// Initial fee pool, in micromels. Half-life is approximately 15 days.
    pub init_fee_pool: u128,
}

impl GenesisConfig {
    /// The "standard" mainnet genesis.
    pub fn std_mainnet() -> Self {
        Self {
            network: NetID::Mainnet,
            init_coindata: CoinData {
                covhash: Covenant::std_ed25519_pk_legacy(Ed25519PK(
                    hex::decode("7323dcb65513b84470a76339cdf0062d47d82e205e834f2d7159684a0cb3b5ba")
                        .unwrap()
                        .try_into()
                        .unwrap(),
                ))
                .hash(),
                value: 1000000 * MICRO_CONVERTER, // 1 million SYM
                denom: Denom::Sym,
                additional_data: vec![],
            },
            stakes: ["7323dcb65513b84470a76339cdf0062d47d82e205e834f2d7159684a0cb3b5ba"]
                .iter()
                .map(|v| Ed25519PK(hex::decode(v).unwrap().try_into().unwrap()))
                .map(|pubkey| {
                    (
                        tmelcrypt::hash_single(&pubkey.0), // A nonexistent hash
                        StakeDoc {
                            pubkey,
                            e_start: 0,
                            e_post_end: 3, // for the first two epochs (140 days)
                            syms_staked: 1,
                        },
                    )
                })
                .collect(),
            init_fee_pool: 6553600 * MICRO_CONVERTER, // 100 mel/day subsidy, decreasing rapidly
        }
    }
    /// The "standard" testnet genesis.
    pub fn std_testnet() -> Self {
        Self {
            network: NetID::Testnet,
            init_coindata: CoinData {
                covhash: Covenant::always_true().hash(),
                value: 1 << 32,
                denom: Denom::Mel,
                additional_data: vec![],
            },
            stakes: [
                "fae1ff56a62639c7959bf200465f4e06291e4e4dbd751cf4d2c13a8a6bea537c",
                "2ae54755b2e98a3059c68334af97b38603032be53bb2a1a3a183ae0f9d3bdaaf",
                "3aa3b5e2d64916a055da79635a4406999b66dfbe25afb10fa306aa01e42308a6",
                "85e374cc3e4dbf47b9a9697126e2e2ae90011b78a54b84adeb2ffe516b79769a",
            ]
            .iter()
            .map(|v| Ed25519PK(hex::decode(v).unwrap().try_into().unwrap()))
            .map(|pubkey| {
                (
                    tmelcrypt::hash_single(&pubkey.0),
                    StakeDoc {
                        pubkey,
                        e_start: 0,
                        e_post_end: 1 << 32,
                        syms_staked: 1,
                    },
                )
            })
            .collect(),
            init_fee_pool: 1 << 64,
        }
    }
}

/// Identifies a network.
#[derive(
    Clone,
    Copy,
    IntoPrimitive,
    TryFromPrimitive,
    Eq,
    PartialEq,
    Debug,
    Serialize_repr,
    Deserialize_repr,
    Hash,
    Arbitrary,
)]
#[repr(u8)]
pub enum NetID {
    Testnet = 0x01,
    Mainnet = 0xff,
}

/// World state of the Themelio blockchain
#[derive(Clone, Debug)]
pub struct State {
    pub network: NetID,

    pub height: u64,
    pub history: SmtMapping<u64, Header>,
    pub coins: SmtMapping<txn::CoinID, txn::CoinDataHeight>,
    pub transactions: SmtMapping<HashVal, txn::Transaction>,

    pub fee_pool: u128,
    pub fee_multiplier: u128,
    pub tips: u128,

    pub dosc_speed: u128,
    pub pools: PoolMapping,

    pub stakes: SmtMapping<HashVal, StakeDoc>,
}

fn read_bts(r: &mut impl Read, n: usize) -> Option<Vec<u8>> {
    let mut buf: Vec<u8> = vec![0; n];
    r.read_exact(&mut buf).ok()?;
    Some(buf)
}

impl State {
    /// Creates a new State from a config
    pub fn genesis(db: &novasmt::Forest, cfg: GenesisConfig) -> Self {
        let empty_tree = db.open_tree(HashVal::default().0).unwrap();
        let mut new_state = Self {
            network: cfg.network,
            height: 0,
            history: SmtMapping::new(empty_tree.clone()),
            coins: SmtMapping::new(empty_tree.clone()),
            transactions: SmtMapping::new(empty_tree.clone()),
            fee_pool: cfg.init_fee_pool,
            fee_multiplier: MICRO_CONVERTER,
            tips: 0,

            dosc_speed: MICRO_CONVERTER,
            pools: SmtMapping::new(empty_tree.clone()),
            stakes: {
                let mut stakes = SmtMapping::new(empty_tree);
                for (k, v) in cfg.stakes.iter() {
                    stakes.insert(*k, *v);
                }
                dbg!(stakes.root_hash());
                stakes
            },
        };
        // init micromels etc
        new_state.coins.insert(
            CoinID::zero_zero(),
            CoinDataHeight {
                height: 0,
                coin_data: cfg.init_coindata,
            },
        );
        new_state
    }

    /// Generates an encoding of the state that, in conjunction with a SMT database, can recover the entire state.
    pub fn partial_encoding(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[self.network.into()]);
        out.extend_from_slice(&self.height.to_be_bytes());
        out.extend_from_slice(&self.history.root_hash());
        out.extend_from_slice(&self.coins.root_hash());
        out.extend_from_slice(&self.transactions.root_hash());

        out.extend_from_slice(&self.fee_pool.to_be_bytes());
        out.extend_from_slice(&self.fee_multiplier.to_be_bytes());
        out.extend_from_slice(&self.tips.to_be_bytes());

        out.extend_from_slice(&self.dosc_speed.to_be_bytes());
        out.extend_from_slice(&self.pools.root_hash());

        out.extend_from_slice(&self.stakes.root_hash());
        out
    }

    /// Restores a state from its partial encoding in conjunction with a database. **Does not validate data and will panic; do not use on untrusted data**
    pub fn from_partial_encoding_infallible(mut encoding: &[u8], db: &novasmt::Forest) -> Self {
        defmac!(readu8 => u8::from_be_bytes(read_bts(&mut encoding, 1).unwrap().as_slice().try_into().unwrap()));
        defmac!(readu64 => u64::from_be_bytes(read_bts(&mut encoding, 8).unwrap().as_slice().try_into().unwrap()));
        defmac!(readu128 => u128::from_be_bytes(read_bts(&mut encoding, 16).unwrap().as_slice().try_into().unwrap()));
        defmac!(readtree => SmtMapping::new(db.open_tree(
            read_bts(&mut encoding, 32).unwrap().as_slice().try_into().unwrap(),
        ).unwrap()));
        let network: NetID = readu8!().try_into().unwrap();
        let height = readu64!();
        let history = readtree!();
        let coins = readtree!();
        let transactions = readtree!();

        let fee_pool = readu128!();
        let fee_multiplier = readu128!();
        let tips = readu128!();

        let dosc_multiplier = readu128!();
        let pools = readtree!();

        let stakes = readtree!();
        State {
            network,
            height,
            history,
            coins,
            transactions,

            fee_pool,
            fee_multiplier,
            tips,

            dosc_speed: dosc_multiplier,
            pools,

            stakes,
        }
    }

    /// Generates a test genesis state, with a given starting coin.
    pub fn test_genesis(
        db: novasmt::Forest,
        start_micro_mels: u128,
        start_cov_hash: tmelcrypt::HashVal,
        start_stakeholders: &[tmelcrypt::Ed25519PK],
    ) -> Self {
        assert!(start_micro_mels <= MAX_COINVAL);
        let mut empty = Self::new_empty_testnet(db);
        // insert coin out of nowhere
        let init_coin = txn::CoinData {
            covhash: start_cov_hash,
            value: start_micro_mels,
            denom: Denom::Mel,
            additional_data: vec![],
        };
        empty.coins.insert(
            txn::CoinID {
                txhash: tmelcrypt::HashVal([0; 32]),
                index: 0,
            },
            txn::CoinDataHeight {
                coin_data: init_coin,
                height: 0,
            },
        );
        for (i, stakeholder) in start_stakeholders.iter().enumerate() {
            empty.stakes.insert(
                tmelcrypt::hash_single(&(i as u128).to_be_bytes()),
                StakeDoc {
                    pubkey: *stakeholder,
                    e_start: 0,
                    e_post_end: 1000000000,
                    syms_staked: 100,
                },
            );
        }
        empty
    }
    /// Applies a single transaction.
    pub fn apply_tx(&mut self, tx: &txn::Transaction) -> Result<(), StateError> {
        self.apply_tx_batch(std::slice::from_ref(tx))
    }

    /// Saves all the SMTs to disk.
    pub fn save_smts(&mut self) {
        self.history.mapping.save();
        self.coins.mapping.save();
        self.pools.mapping.save();
        self.transactions.mapping.save();
        self.stakes.mapping.save();
    }

    pub fn apply_tx_batch(&mut self, txx: &[txn::Transaction]) -> Result<(), StateError> {
        let old_hash = self.coins.root_hash();
        StateHandle::new(self).apply_tx_batch(&txx)?.commit();
        log::debug!(
            "applied a batch of {} txx to {:?} => {:?}",
            txx.len(),
            old_hash,
            self.coins.root_hash()
        );
        Ok(())
    }

    /// Finalizes a state into a block. This consumes the state.
    pub fn seal(mut self, action: Option<ProposerAction>) -> SealedState {
        // first apply melmint
        self = preseal_melmint(self);
        assert!(self.pools.val_iter().count() >= 2);

        let after_tip_901 = self.height >= 42700;

        // apply the proposer action
        if let Some(action) = action {
            // first let's move the fee multiplier
            let max_movement = if after_tip_901 {
                ((self.fee_multiplier >> 7) as i64).max(2)
            } else {
                (self.fee_multiplier >> 7) as i64
            };
            let scaled_movement = max_movement * action.fee_multiplier_delta as i64 / 128;
            log::debug!(
                "changing fee multiplier {} by {}",
                self.fee_multiplier,
                scaled_movement
            );
            if scaled_movement >= 0 {
                self.fee_multiplier += scaled_movement as u128;
            } else {
                self.fee_multiplier -= scaled_movement.abs() as u128;
            }

            // then it's time to collect the fees dude! we synthesize a coin with 1/65536 of the fee pool and all the tips.
            let base_fees = self.fee_pool >> 16;
            self.fee_pool -= base_fees;
            let tips = self.tips;
            self.tips = 0;
            let pseudocoin_id = CoinID::proposer_reward(self.height);
            let pseudocoin_data = CoinDataHeight {
                coin_data: CoinData {
                    covhash: action.reward_dest,
                    value: base_fees + tips,
                    denom: Denom::Mel,
                    additional_data: vec![],
                },
                height: self.height,
            };
            // insert the fake coin
            self.coins.insert(pseudocoin_id, pseudocoin_data);
        }
        // create the finalized state
        SealedState(self, action)
    }

    // ----------- helpers start here ------------

    pub(crate) fn new_empty_testnet(db: novasmt::Forest) -> Self {
        let empty_tree = db.open_tree(Default::default()).unwrap();
        State {
            network: NetID::Testnet,
            height: 0,
            history: SmtMapping::new(empty_tree.clone()),
            coins: SmtMapping::new(empty_tree.clone()),
            transactions: SmtMapping::new(empty_tree.clone()),
            fee_pool: 1000000,
            fee_multiplier: 1000,
            dosc_speed: 1,
            tips: 0,
            pools: SmtMapping::new(empty_tree.clone()),
            stakes: SmtMapping::new(empty_tree),
        }
    }
}

/// SealedState represents an immutable state at a finalized block height.
/// It cannot be constructed except through sealing a State or restoring from persistent storage.
#[derive(Clone, Debug)]
pub struct SealedState(State, Option<ProposerAction>);

impl SealedState {
    /// Forcibly creates a SealedState.
    pub(crate) fn force_new(state: State) -> Self {
        Self(state, None)
    }

    /// Returns a reference to the State finalized within.
    pub fn inner_ref(&self) -> &State {
        &self.0
    }

    /// Saves the inner state to disk.
    pub fn save_smts(&mut self) {
        self.0.save_smts()
    }

    /// Returns whether or not it's empty.
    pub fn is_empty(&self) -> bool {
        self.1.is_none() && self.inner_ref().transactions.root_hash() == Default::default()
    }

    /// Returns the **partial** encoding, which must be combined with a SMT database to reconstruct the actual state.
    pub fn partial_encoding(&self) -> Vec<u8> {
        let tmp = (self.0.partial_encoding(), &self.1);
        stdcode::serialize(&tmp).unwrap()
    }

    /// Decodes from the partial encoding.
    pub fn from_partial_encoding_infallible(bts: &[u8], db: &novasmt::Forest) -> Self {
        let tmp: (Vec<u8>, Option<ProposerAction>) = stdcode::deserialize(&bts).unwrap();
        SealedState(State::from_partial_encoding_infallible(&tmp.0, db), tmp.1)
    }

    /// Returns the block header represented by the finalized state.
    pub fn header(&self) -> Header {
        let inner = &self.0;
        // panic!()
        Header {
            network: inner.network,
            previous: (inner.height.checked_sub(1))
                .map(|height| inner.history.get(&height).0.unwrap().hash())
                .unwrap_or_default(),
            height: inner.height,
            history_hash: inner.history.root_hash(),
            coins_hash: inner.coins.root_hash(),
            transactions_hash: inner.transactions.root_hash(),
            fee_pool: inner.fee_pool,
            fee_multiplier: inner.fee_multiplier,
            dosc_speed: inner.dosc_speed,
            pools_hash: inner.pools.root_hash(),
            stakes_hash: inner.stakes.root_hash(),
        }
    }

    /// Returns the proposer action.
    pub fn proposer_action(&self) -> Option<&ProposerAction> {
        self.1.as_ref()
    }

    /// Returns the final state represented as a "block" (header + transactions).
    pub fn to_block(&self) -> Block {
        let mut txx = im::HashSet::new();
        for tx in self.0.transactions.val_iter() {
            txx.insert(tx);
        }
        // self check since im sometimes is buggy
        for tx in self.0.transactions.val_iter() {
            assert!(txx.contains(&tx));
        }
        Block {
            header: self.header(),
            transactions: txx,
            proposer_action: self.1,
        }
    }
    /// Creates a new unfinalized state representing the next block.
    pub fn next_state(&self) -> State {
        let mut new = self.inner_ref().clone();
        // fee variables
        new.history.insert(self.0.height, self.header());
        new.height += 1;
        new.stakes.remove_stale(new.height / STAKE_EPOCH);
        new.transactions.clear();
        new
    }

    /// Applies a block to this state.
    pub fn apply_block(&self, block: &Block) -> Result<SealedState, StateError> {
        let mut basis = self.next_state();
        assert!(basis.pools.val_iter().count() >= 2);
        let transactions = block.transactions.iter().cloned().collect::<Vec<_>>();
        basis.apply_tx_batch(&transactions)?;
        assert!(basis.pools.val_iter().count() >= 2);
        let basis = basis.seal(block.proposer_action);
        assert!(basis.inner_ref().pools.val_iter().count() >= 2);
        if basis.header() != block.header {
            log::warn!(
                "post-apply header {:#?} doesn't match declared header {:#?} with {} txx",
                basis.header(),
                block.header,
                transactions.len()
            );
            assert!(basis.inner_ref().pools.val_iter().count() >= 2);
            for pool_info in basis.inner_ref().pools.val_iter() {
                dbg!(pool_info);
            }
            return Err(StateError::WrongHeader);
        }
        Ok(basis)
    }

    /// Confirms a state with a given consensus proof. If called with a second argument, this function is supposed to be called to *verify* the consensus proof.
    ///
    /// **TODO**: Right now it DOES NOT check the consensus proof!
    pub fn confirm(
        self,
        cproof: ConsensusProof,
        previous_state: Option<&State>,
    ) -> Option<ConfirmedState> {
        Some(ConfirmedState {
            state: self,
            cproof,
        })
    }
}

/// ProposerAction describes the standard action that the proposer takes when proposing a block.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
pub struct ProposerAction {
    /// Change in fee. This is scaled to the proper size.
    pub fee_multiplier_delta: i8,
    /// Where to sweep fees.
    pub reward_dest: HashVal,
}

pub type ConsensusProof = BTreeMap<Ed25519PK, Vec<u8>>;

/// ConfirmedState represents a fully confirmed state with a consensus proof.
#[derive(Clone, Debug)]
pub struct ConfirmedState {
    state: SealedState,
    cproof: ConsensusProof,
}

impl ConfirmedState {
    /// Returns the wrapped finalized state
    pub fn inner(&self) -> &SealedState {
        &self.state
    }

    /// Returns the proof
    pub fn cproof(&self) -> &ConsensusProof {
        &self.cproof
    }
}

// impl Deref<Target =
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash, Arbitrary)]
/// A block header, which commits to a particular SealedState.
pub struct Header {
    pub network: NetID,
    pub previous: HashVal,
    pub height: u64,
    pub history_hash: HashVal,
    pub coins_hash: HashVal,
    pub transactions_hash: HashVal,
    pub fee_pool: u128,
    pub fee_multiplier: u128,
    pub dosc_speed: u128,
    pub pools_hash: HashVal,
    pub stakes_hash: HashVal,
}

impl Header {
    pub fn hash(&self) -> tmelcrypt::HashVal {
        tmelcrypt::hash_single(&stdcode::serialize(self).unwrap())
    }

    pub fn validate_cproof(
        &self,
        _cproof: &ConsensusProof,
        previous_state: Option<&State>,
    ) -> bool {
        if previous_state.is_none() && self.height != 0 {
            return false;
        }
        // TODO
        true
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
/// A (serialized) block.
pub struct Block {
    pub header: Header,
    pub transactions: im::HashSet<Transaction>,
    pub proposer_action: Option<ProposerAction>,
}

impl Block {
    /// Abbreviate a block
    pub fn abbreviate(&self) -> AbbrBlock {
        AbbrBlock {
            header: self.header,
            txhashes: self.transactions.iter().map(|v| v.hash_nosigs()).collect(),
            proposer_action: self.proposer_action,
        }
    }
}

/// An abbreviated block
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AbbrBlock {
    pub header: Header,
    pub txhashes: BTreeSet<HashVal>,
    pub proposer_action: Option<ProposerAction>,
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::testing::fixtures::valid_txx;
    use crate::Transaction;
    use rstest::*;

    #[rstest]
    #[ignore]
    fn test_apply_tx_batch_not_well_formed_errors() {
        // create a batch of transactions
        // ensure at least one of them is not well formed
        // call apply tx batch
        // verify you get a state error
    }

    #[rstest]
    #[ignore]
    fn test_apply_tx_batch(valid_txx: Vec<Transaction>) {
        // create a batch of transactions
        // valid_txx()
        // call apply tx batch

        // verify result is ok
    }
    // TODO: add tests for State::seal & SealedState methods
}
