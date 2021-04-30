#![allow(clippy::upper_case_acronyms)]

use arbitrary::Arbitrary;
use ed25519_dalek::{Signer, Verifier};
use fmt::Display;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use serde_big_array::big_array;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::{convert::TryFrom, str::FromStr};
use std::{convert::TryInto, fmt::Formatter};

big_array! { BigArray; }

#[derive(
    Copy, Clone, Eq, PartialEq, Hash, Arbitrary, Ord, PartialOrd, Default, Serialize, Deserialize,
)]
/// Represents an 256-byte hash value.
#[serde(transparent)]
pub struct HashVal(#[serde(with = "stdcode::hex32")] pub [u8; 32]);

impl FromStr for HashVal {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut inner = [0u8; 32];
        hex::decode_to_slice(s, &mut inner)?;
        Ok(Self(inner))
    }
}

impl Display for HashVal {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        hex::encode(&self.0).fmt(f)
    }
}

impl HashVal {
    /// Randomly generates a HashVal. This will almost certainly not collide with the actual hash of anything.
    pub fn random() -> Self {
        HashVal(rand::thread_rng().gen())
    }

    pub fn to_addr(&self) -> String {
        let raw_base32 = base32::encode(base32::Alphabet::Crockford {}, &self.0);
        let checksum = hash_keyed(b"address-checksum", &self.0).0[0] % 10;
        format!("T{}{}", checksum, raw_base32).to_ascii_lowercase()
        // res.into_bytes()
        //     .chunks(5)
        //     .map(|chunk| String::from_utf8_lossy(chunk).to_ascii_lowercase())
        //     .collect::<Vec<_>>()
        //     .join("-")
    }

    pub fn from_addr(addr: &str) -> Option<Self> {
        // TODO check checksum
        if addr.len() < 10 {
            return None;
        }
        let addr = addr.replace("-", "");
        Some(HashVal(
            base32::decode(base32::Alphabet::Crockford {}, &addr[2..])?
                .as_slice()
                .try_into()
                .ok()?,
        ))
    }
}

/// Computes an entropy seed from a large number of hashes using the "majority beacon".
pub fn majority_beacon(elems: &[HashVal]) -> HashVal {
    let bts: Vec<u8> = (0..32)
        .map(|i| {
            let bytes: Vec<u8> = elems.iter().map(|v| v[i]).collect();
            bitwise_majority(&bytes)
        })
        .collect();
    HashVal(bts.try_into().unwrap())
}

// helper function that takes the bitwise majority of a large number of u8's
fn bitwise_majority(bytes: &[u8]) -> u8 {
    let mut toret = 0u8;
    for bit_idx in 0..8 {
        let mut zero_count = 0;
        let mut one_count = 0;
        for member in bytes {
            if member & (1 << bit_idx) == 1 {
                one_count += 1;
            } else {
                zero_count += 1;
            }
        }
        assert_eq!(zero_count + one_count, bytes.len());
        if one_count > zero_count {
            toret |= 1 << bit_idx;
        }
    }
    toret
}

impl Deref for HashVal {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<[u8]> for HashVal {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for HashVal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("#<{}>", hex::encode(&self.0)))
    }
}

/// Hashes a single value.
pub fn hash_single(val: impl AsRef<[u8]>) -> HashVal {
    let b3h = blake3::hash(val.as_ref());
    HashVal((*b3h.as_bytes().as_ref()).try_into().unwrap())
}

/// Hashes a value with the given key.
pub fn hash_keyed(key: impl AsRef<[u8]>, val: impl AsRef<[u8]>) -> HashVal {
    let b3h = blake3::keyed_hash(&hash_single(key).0, val.as_ref());
    HashVal((*b3h.as_bytes().as_ref()).try_into().unwrap())
}

/// Generates an ed25519 keypair.
pub fn ed25519_keygen() -> (Ed25519PK, Ed25519SK) {
    let mut csprng = rand::thread_rng();
    let keypair = ed25519_dalek::Keypair::generate(&mut csprng);
    (
        Ed25519PK(keypair.public.to_bytes()),
        Ed25519SK(keypair.to_bytes()),
    )
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
/// An ed25519 public key.
#[serde(transparent)]
pub struct Ed25519PK(#[serde(with = "stdcode::hex32")] pub [u8; 32]);

impl Ed25519PK {
    pub fn verify(&self, msg: &[u8], sig: &[u8]) -> bool {
        let pk = ed25519_dalek::PublicKey::from_bytes(&self.0);
        match pk {
            Ok(pk) => match ed25519_dalek::Signature::try_from(sig) {
                Ok(sig) => pk.verify(msg, &sig).is_ok(),
                Err(_) => false,
            },
            Err(_) => false,
        }
    }

    pub fn from_bytes(bts: &[u8]) -> Option<Self> {
        if bts.len() != 32 {
            None
        } else {
            let mut buf = [0; 32];
            buf.copy_from_slice(bts);
            Some(Ed25519PK(buf))
        }
    }
}

impl fmt::Debug for Ed25519PK {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("#<EdPK:{}>", hex::encode(&self.0[..5])))
    }
}
#[derive(Copy, Clone, Serialize, Deserialize)]
/// An ed25519 secret key. Implements FromStr that converts from hexadecimal.
pub struct Ed25519SK(#[serde(with = "BigArray")] pub [u8; 64]);

impl FromStr for Ed25519SK {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let vv = hex::decode(s)?;
        Ok(Ed25519SK(
            vv.try_into()
                .map_err(|_| hex::FromHexError::InvalidStringLength)?,
        ))
    }
}

impl PartialEq for Ed25519SK {
    fn eq(&self, other: &Self) -> bool {
        let x = &self.0[0..];
        let y = &other.0[0..];
        x == y
    }
}

impl Eq for Ed25519SK {}

impl Hash for Ed25519SK {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for k in self.0.iter() {
            k.hash(state);
        }
    }
}

impl Ed25519SK {
    pub fn sign(&self, msg: &[u8]) -> Vec<u8> {
        let kp = ed25519_dalek::Keypair::from_bytes(&self.0).unwrap();
        kp.sign(msg).to_bytes().to_vec()
    }

    pub fn from_bytes(bts: &[u8]) -> Option<Self> {
        if bts.len() != 64 {
            None
        } else {
            let mut buf = [0; 64];
            let kp = ed25519_dalek::Keypair::from_bytes(&bts);
            if kp.is_err() {
                None
            } else {
                buf.copy_from_slice(bts);
                Some(Ed25519SK(buf))
            }
        }
    }

    pub fn to_public(&self) -> Ed25519PK {
        let kp = ed25519_dalek::Keypair::from_bytes(&self.0).unwrap();
        Ed25519PK(kp.public.to_bytes())
    }
}

impl fmt::Debug for Ed25519SK {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("EdSK({})", hex::encode(self.0.as_ref())))
    }
}
