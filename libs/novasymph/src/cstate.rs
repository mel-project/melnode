mod helpers;
use novasmt::ContentAddrStore;
use themelio_stf::SealedState;
pub mod gossip;
use crate::blockgraph::BlockGraph;
use themelio_stf::StakeMapping;
use themelio_structs::BlockHeight;

/// A representation of the chain state internal to Symphonia.
pub struct ChainState<C: ContentAddrStore> {
    epoch: u64,
    stakes: StakeMapping<C>,
    pub blockgraph: BlockGraph<C>,

    drained_height: BlockHeight,
}

impl<C: ContentAddrStore> ChainState<C> {
    /// Create a new ChainState with the given genesis state.
    pub fn new(genesis: SealedState<C>, blockgraph: BlockGraph<C>) -> Self {
        let epoch = genesis.inner_ref().height.epoch();
        let stakes = genesis.inner_ref().stakes.clone();

        Self {
            epoch,
            stakes,
            blockgraph,

            drained_height: 0.into(),
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use themelio_stf::GenesisConfig;

//     use super::*;

//     #[test]
//     fn simple_sequence() {
//         let forest = novasmt::Database::new(novasmt::InMemoryCas::default());
//         let genesis = GenesisConfig::std_testnet().realize(&forest).seal(None);
//         let cstate = ChainState::new(genesis, forest);
//         dbg!(cstate.get_lnc_tips());
//     }
// }
