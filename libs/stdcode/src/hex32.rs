use std::convert::TryInto;

use serde::Deserialize;
use serde::Serialize;
use serde::{Deserializer, Serializer};

pub fn serialize<S>(bytes: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if serializer.is_human_readable() {
        serializer.serialize_str(&hex::encode(bytes))
    } else {
        bytes.serialize(serializer)
    }

    // Could also use a wrapper type with a Display implementation to avoid
    // allocating the String.
    //
    // serializer.collect_str(&Base64(bytes))
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
where
    D: Deserializer<'de>,
{
    if deserializer.is_human_readable() {
        let s = <&str>::deserialize(deserializer)?;
        hex::decode(s)
            .map_err(serde::de::Error::custom)?
            .try_into()
            .map_err(|_| serde::de::Error::custom("hexadecimal length not right"))
    } else {
        <[u8; 32]>::deserialize(deserializer)
    }
    // let s = <&str>::deserialize(deserializer)?;
    // base64::decode(s).map_err(de::Error::custom)
}
