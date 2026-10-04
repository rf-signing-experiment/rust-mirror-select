use std::fmt;
use std::str::FromStr;

use base64::prelude::*;

use crate::{Error, Mirror};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirrorEntry(Vec<u8>);

impl MirrorEntry {
    pub fn encode(mirror: &Mirror) -> Result<Self, Error> {
        Ok(Self(serde_json::to_vec(mirror)?))
    }

    pub fn decode(&self) -> Result<Mirror, Error> {
        Ok(serde_json::from_slice(&self.0)?)
    }

    pub fn from_base64(text: impl AsRef<[u8]>) -> Result<Self, Error> {
        Ok(Self(BASE64_STANDARD.decode(text)?))
    }

    pub fn to_base64(&self) -> String {
        BASE64_STANDARD.encode(&self.0)
    }
}

impl FromStr for MirrorEntry {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_base64(s)
    }
}

impl fmt::Display for MirrorEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_base64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MirrorContents;

    fn mirror() -> Mirror {
        Mirror::new(
            "https://static.rust-lang.org/".parse().unwrap(),
            MirrorContents::RUSTUP | MirrorContents::TOOLCHAIN,
        )
    }

    #[test]
    fn entry_round_trips_through_base64_text() {
        let entry = MirrorEntry::encode(&mirror()).unwrap();
        let text = entry.to_string();
        assert!(text.is_ascii() && !text.contains(['"', ' ']));

        let parsed: MirrorEntry = text.parse().unwrap();
        assert_eq!(parsed, entry);
        assert_eq!(parsed.decode().unwrap(), mirror());
    }

    #[test]
    fn rejects_invalid_base64() {
        assert!(matches!(
            MirrorEntry::from_base64("not base64!"),
            Err(Error::Base64(_))
        ));
    }

    #[test]
    fn rejects_payload_that_is_not_a_mirror() {
        let entry = MirrorEntry::from_base64(BASE64_STANDARD.encode(b"{\"V9\":{}}")).unwrap();
        assert!(matches!(entry.decode(), Err(Error::Mirror(_))));
    }
}
