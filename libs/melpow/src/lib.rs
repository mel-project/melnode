//! `melpow` is the crate that implements MelPoW, Themelio's version of non-interactive proofs of sequential work, which are just "Interactive Proofs of Sequential Work" by Cohen and Pietrzak subjected to a Fiat-Shamir transformation. MelPoW is used as the core mechanism behind Melmint, the algorithmic monetary policy system that stabilizes the mel.
//!
//! `Proof` is the main interface to MelPoW. It represents a proof that a certain amount of sequential work, represented by a **difficulty**, has been done starting from a **puzzle**. The difficulty is exponential: a difficulty of N represents that `O(2^N)` work has been done.

mod hash;
mod node;
use std::convert::TryInto;

use node::SVec;
use rustc_hash::FxHashMap;

const PROOF_CERTAINTY: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
/// A MelPoW proof with an opaque representation that is guaranteed to be stable. It can be cloned relatively cheaply because it's internally reference counted.
pub struct Proof(im::HashMap<node::Node, SVec<u8>>);

impl Proof {
    /// Generates a MelPoW proof with respect to the given starting puzzle and a difficulty.
    pub fn generate(puzzle: &[u8], difficulty: usize) -> Self {
        let mut proof_map = FxHashMap::default();
        let chi = hash::bts_key(puzzle, b"chi");
        let gammas = gen_gammas(puzzle, difficulty);
        for g in gammas {
            for pn in gamma_to_path(g) {
                proof_map.insert(pn, SVec::new());
            }
            proof_map.insert(g, SVec::new());
        }
        node::calc_labels(&chi, difficulty, &mut |nd, lab| {
            if proof_map.get(&nd).is_some() || nd.len == 0 {
                proof_map.insert(nd, SVec::from_slice(lab));
            }
        });
        Proof(proof_map.into_iter().collect())
    }
    /// Verifies a MelPoW proof.
    #[must_use]
    pub fn verify(&self, puzzle: &[u8], difficulty: usize) -> bool {
        if difficulty > 100 {
            return false;
        }
        let chi = hash::bts_key(puzzle, b"chi");
        let gammas = gen_gammas(puzzle, difficulty);
        let phi = self.0[&node::Node::new_zero()].clone();
        let mut temp_map = self.0.clone();
        for gamma in gammas {
            match self.0.get(&gamma) {
                None => {
                    return false;
                }
                Some(label) => {
                    // verify that the label is correctly calculated from parents
                    let mut hasher = hash::Accumulator::new(&chi);
                    hasher.add(&gamma.to_bytes());
                    for parent in gamma.get_parents(difficulty) {
                        match self.0.get(&parent) {
                            None => return false,
                            Some(parlab) => {
                                hasher.add(&parlab);
                            }
                        }
                    }
                    if hasher.hash() != *label {
                        return false;
                    }
                    // check "merkle-like" commitment
                    for i in (0..difficulty).rev() {
                        let mut h = hash::Accumulator::new(&chi);
                        h.add(&gamma.take(i).to_bytes());
                        let g_l_0 = gamma.take(i).append(0);
                        let g_l_1 = gamma.take(i).append(1);
                        let g_l = gamma.take(i);
                        let h = h.add(&temp_map[&g_l_0]).add(&temp_map[&g_l_1]).hash();
                        temp_map.insert(g_l, h);
                    }
                    if phi != self.0[&node::Node::new_zero()].clone() {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Serializes the proof to a byte vector.
    pub fn to_bytes(&self) -> Vec<u8> {
        let unit_size = 8 + 32;
        let mut output = Vec::with_capacity(unit_size * self.0.len());
        for (k, v) in self.0.iter() {
            assert_eq!(v.len(), 32);
            output.extend_from_slice(&k.to_bytes());
            output.extend_from_slice(&v);
        }
        output
    }

    /// Deserializes a proof from a byte vector.
    pub fn from_bytes(mut bts: &[u8]) -> Option<Self> {
        let unit_size = 8 + 32;
        if bts.len() % unit_size != 0 {
            return None;
        }
        let mut omap = im::HashMap::new();
        while !bts.is_empty() {
            let nd = node::Node::from_bytes(&bts[0..8])?;
            let lab = SVec::from_slice(&bts[8..32 + 8]);
            omap.insert(nd, lab);
            bts = &bts[unit_size..]
        }
        Some(Proof(omap))
    }
}

fn gen_gammas(puzzle: &[u8], difficulty: usize) -> Vec<node::Node> {
    (0..PROOF_CERTAINTY)
        .map(|i| {
            let g_seed = hash::bts_key(puzzle, format!("gamma-{}", i).as_bytes());
            let g_int = u64::from_le_bytes(g_seed[0..8].try_into().unwrap());
            let shift = 64 - difficulty;
            let g_int = (g_int >> shift) << shift;
            let g_int = g_int.reverse_bits();
            node::Node::new(g_int, difficulty)
        })
        .collect()
}

fn gamma_to_path(gamma: node::Node) -> Vec<node::Node> {
    let n = gamma.len;
    (0..n)
        .map(|i| gamma.take(i).append(1 - gamma.get_bit(i) as usize))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple() {
        let difficulty = 8;
        let puzzle = vec![];
        let proof = Proof::generate(&puzzle, difficulty);
        assert!(proof.verify(&puzzle, difficulty));
        assert!(!proof.verify(&puzzle, difficulty + 1));
        assert!(!proof.verify(b"hello", difficulty));
        assert_eq!(Proof::from_bytes(&proof.to_bytes()).unwrap(), proof);
        println!("proof length is {}", proof.to_bytes().len())
    }
}
