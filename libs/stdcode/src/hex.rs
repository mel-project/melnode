use serde::Deserialize;
use serde::{Deserializer, Serializer};

pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if serializer.is_human_readable() {
        serializer.serialize_str(&hex::encode(bytes))
    } else {
        serializer.serialize_bytes(&bytes)
    }

    // Could also use a wrapper type with a Display implementation to avoid
    // allocating the String.
    //
    // serializer.collect_str(&Base64(bytes))
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    if deserializer.is_human_readable() {
        let s = <&str>::deserialize(deserializer)?;
        hex::decode(s).map_err(serde::de::Error::custom)
    } else {
        <Vec<u8>>::deserialize(deserializer)
    }
    // let s = <&str>::deserialize(deserializer)?;
    // base64::decode(s).map_err(de::Error::custom)
}
