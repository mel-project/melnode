use blkstructs::AbbrBlock;
use serde::{Deserialize, Serialize};
use tmelcrypt::{Ed25519PK, Ed25519SK};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProposalSig(Vec<u8>);

impl ProposalSig {
    /// Verify that this is a valid proposal for a particular AbbrBlock.
    pub fn verify(&self, proposer: Ed25519PK, abbr: &AbbrBlock) -> bool {
        let abbr_bytes = stdcode::serialize(abbr).unwrap();
        proposer.verify(
            &tmelcrypt::hash_keyed(b"symph_prop_sig", &abbr_bytes),
            &self.0,
        )
    }

    /// Generate a signature.
    pub fn generate(proposer_sk: Ed25519SK, abbr: &AbbrBlock) -> Self {
        let to_sign = tmelcrypt::hash_keyed(b"symph_prop_sig", &stdcode::serialize(abbr).unwrap());
        Self(proposer_sk.sign(&to_sign))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VoteSig(Vec<u8>);

impl VoteSig {
    /// Verify that this is a valid proposal for a particular AbbrBlock.
    pub fn verify(&self, voter: Ed25519PK, abbr: &AbbrBlock) -> bool {
        let abbr_bytes = stdcode::serialize(abbr).unwrap();
        voter.verify(
            &tmelcrypt::hash_keyed(b"symph_vote_sig", &abbr_bytes),
            &self.0,
        )
    }

    /// Generate a signature.
    pub fn generate(my_sk: Ed25519SK, abbr: &AbbrBlock) -> Self {
        let to_sign = tmelcrypt::hash_keyed(b"symph_vote_sig", &stdcode::serialize(abbr).unwrap());
        Self(my_sk.sign(&to_sign))
    }
}
