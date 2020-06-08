use rlp::{Decodable, Encodable};
use std::convert::TryInto;
use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct HashVal(pub [u8; 32]);

impl fmt::Debug for HashVal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("HashVal({})", hex::encode(&self.0)))
    }
}

impl Encodable for HashVal {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        let arr = self.0.as_ref();
        arr.rlp_append(s)
    }
}

impl Decodable for HashVal {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let v = Vec::decode(r)?;
        if v.len() != 32 {
            Err(rlp::DecoderError::Custom("HashVal not 32 bytes"))
        } else {
            let v = v.as_slice();
            let v = v.try_into().unwrap();
            Ok(HashVal(v))
        }
    }
}

pub fn hash_single(val: &[u8]) -> HashVal {
    let b3h = blake3::hash(val);
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

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Ed25519PK(pub [u8; 32]);

impl Ed25519PK {
    pub fn verify(&self, msg: &[u8], sig: &[u8]) -> bool {
        let pk = ed25519_dalek::PublicKey::from_bytes(&self.0).unwrap();
        pk.verify(msg, &ed25519_dalek::Signature::from_bytes(sig).unwrap())
            .is_ok()
    }

    pub fn from_bytes(bts: &[u8]) -> Option<Self> {
        if bts.len() != 64 {
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
        f.write_fmt(format_args!("Ed25519PK({})", hex::encode(&self.0)))
    }
}

impl Encodable for Ed25519PK {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        let arr = self.0.as_ref();
        arr.rlp_append(s)
    }
}

impl Decodable for Ed25519PK {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let v = Vec::decode(r)?;
        if v.len() != 32 {
            Err(rlp::DecoderError::Custom("Ed25519PK not 32 bytes"))
        } else {
            let v = v.as_slice();
            let v = v.try_into().unwrap();
            Ok(Ed25519PK(v))
        }
    }
}

#[derive(Copy, Clone)]
pub struct Ed25519SK(pub [u8; 64]);

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
            buf.copy_from_slice(bts);
            Some(Ed25519SK(buf))
        }
    }
}

impl fmt::Debug for Ed25519SK {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("Ed25519PK({})", hex::encode(self.0.as_ref())))
    }
}

impl Encodable for Ed25519SK {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        let arr = self.0.as_ref();
        arr.rlp_append(s)
    }
}

impl Decodable for Ed25519SK {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let v = Vec::decode(r)?;
        if v.len() != 64 {
            Err(rlp::DecoderError::Custom("Ed25519SK not 64 bytes"))
        } else {
            let v = v.as_slice();
            let mut w = [0; 64];
            w.clone_from_slice(v);
            Ok(Ed25519SK(w))
        }
    }
}
