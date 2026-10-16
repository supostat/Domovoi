use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer};

const DIGEST_BYTES: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sha256Digest([u8; DIGEST_BYTES]);

impl Sha256Digest {
    pub fn from_bytes(bytes: [u8; DIGEST_BYTES]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; DIGEST_BYTES] {
        &self.0
    }
}

impl FromStr for Sha256Digest {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let is_lowercase_hex = text.len() == DIGEST_BYTES * 2
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if !is_lowercase_hex {
            return Err(format!("{text:?} is not 64 lowercase hexadecimal digits"));
        }
        let mut bytes = [0_u8; DIGEST_BYTES];
        for (position, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&text[position * 2..position * 2 + 2], 16)
                .expect("the digits were checked to be hexadecimal");
        }
        Ok(Self(bytes))
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Sha256Digest({self})")
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}
