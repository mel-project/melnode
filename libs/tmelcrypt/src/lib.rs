use arbitrary::Arbitrary;
use serde::{Deserialize, Serialize};
use serde_big_array::big_array;
use std::convert::TryInto;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use ed25519_dalek::{Signer, Verifier};
use std::convert::TryFrom;

big_array! { BigArray; }

#[derive(
    Copy, Clone, Eq, PartialEq, Hash, Arbitrary, Ord, PartialOrd, Default, Serialize, Deserialize,
)]
pub struct HashVal(pub [u8; 32]);

impl HashVal {
    pub fn to_addr(&self) -> String {
        let raw_base32 = base32::encode(base32::Alphabet::Crockford {}, &self.0);
        let checksum = hash_keyed(b"address-checksum", &self.0).0[0] % 10;
        let res = format!("T{}{}", checksum, raw_base32);
        res.into_bytes()
            .chunks(5)
            .map(|chunk| String::from_utf8_lossy(chunk).to_ascii_lowercase())
            .collect::<Vec<_>>()
            .join("-")
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
        f.write_fmt(format_args!("#<{}>", hex::encode(&self.0[0..5])))
    }
}

pub fn hash_single(val: &[u8]) -> HashVal {
    let b3h = blake3::hash(val);
    HashVal((*b3h.as_bytes().as_ref()).try_into().unwrap())
}

pub fn hash_keyed(key: &[u8], val: &[u8]) -> HashVal {
    let b3h = blake3::keyed_hash(&hash_single(key).0, val);
    HashVal((*b3h.as_bytes().as_ref()).try_into().unwrap())
}

pub fn ed25519_keygen() -> (Ed25519PK, Ed25519SK) {
    let mut csprng = rand::thread_rng();
    let keypair = ed25519_dalek::Keypair::generate(&mut csprng);
    (
        Ed25519PK(keypair.public.to_bytes()),
        Ed25519SK(keypair.to_bytes()),
    )
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Ed25519PK(pub [u8; 32]);

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
        f.write_fmt(format_args!("EdPK({})", hex::encode(&self.0)))
    }
}
#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Ed25519SK(#[serde(with = "BigArray")] pub [u8; 64]);

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
