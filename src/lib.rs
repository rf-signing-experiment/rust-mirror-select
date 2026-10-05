pub mod consts;
#[cfg(feature = "dns")]
pub mod dns;
pub mod encoding;
#[cfg(feature = "ping-select")]
pub mod ping_select;
pub mod set;
#[cfg(feature = "speedtest-select")]
pub mod speedtest_select;
#[cfg(feature = "toml")]
pub mod toml;

use serde::{Deserialize, Serialize};
use url::Url;

#[cfg(feature = "dns")]
pub use dns::{MirrorSelectClient, SelectResolver};
pub use encoding::MirrorEntry;
#[cfg(feature = "ping-select")]
pub use ping_select::PingSelect;
pub use set::{MirrorSet, SelectStrategy};
#[cfg(feature = "speedtest-select")]
pub use speedtest_select::SpeedtestSelect;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("mirror entry is not valid base64")]
    Base64(#[from] base64::DecodeError),
    #[error("mirror entry does not describe a mirror")]
    Mirror(#[from] serde_json::Error),
    #[cfg(any(feature = "ping-select", feature = "speedtest-select"))]
    #[error("no mirror responded")]
    NoReachableMirror,
    #[cfg(feature = "speedtest-select")]
    #[error("HTTP client could not be built")]
    Http(#[from] reqwest::Error),
    #[cfg(feature = "toml")]
    #[error("mirror TOML could not be parsed")]
    TomlParse(#[from] ::toml::de::Error),
    #[cfg(feature = "toml")]
    #[error("mirror set could not be written as TOML")]
    TomlWrite(#[from] ::toml::ser::Error),
    #[cfg(feature = "dns")]
    #[error("malformed DNS message")]
    Dns(#[from] simple_dns::SimpleDnsError),
    #[cfg(feature = "dns")]
    #[error("DNS transport failed")]
    Io(#[from] std::io::Error),
    #[cfg(feature = "dns")]
    #[error("no DNS servers are configured on this system")]
    NoNameservers,
    #[cfg(all(feature = "dns", unix))]
    #[error("could not parse resolv.conf")]
    ResolvConf(#[from] resolv_conf::ParseError),
    #[cfg(feature = "dns")]
    #[error("no DNS response from {server} within {timeout:?}")]
    Timeout {
        server: std::net::SocketAddr,
        timeout: std::time::Duration,
    },
    #[cfg(feature = "dns")]
    #[error("DNS query for {name} answered with {rcode:?}")]
    Rcode {
        name: String,
        rcode: simple_dns::RCODE,
    },
    #[cfg(feature = "dns")]
    #[error("TXT record is not valid UTF-8 text")]
    Text(#[from] std::string::FromUtf8Error),
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    #[repr(transparent)]
    pub struct MirrorContents: u8 {
        const TUF = 1 << 0;
        const RUSTUP = 1 << 1;
        const TOOLCHAIN = 1 << 2;
        const COMPONENT = 1 << 3;
    }
}

impl Serialize for MirrorContents {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.bits())
    }
}

impl<'de> Deserialize<'de> for MirrorContents {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bits = u8::deserialize(deserializer)?;
        Self::from_bits(bits).ok_or_else(|| {
            serde::de::Error::custom(format!("unknown mirror contents bits {bits:#010b}"))
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum Mirror {
    V1 { url: Url, contains: MirrorContents },
}

impl Mirror {
    pub fn new(url: Url, contains: MirrorContents) -> Self {
        Mirror::V1 { url, contains }
    }

    pub fn url(&self) -> &Url {
        match self {
            Mirror::V1 { url, .. } => url,
        }
    }

    pub fn contains(&self) -> MirrorContents {
        match self {
            Mirror::V1 { contains, .. } => *contains,
        }
    }

    pub fn has_tuf(&self) -> bool {
        self.contains().contains(MirrorContents::TUF)
    }

    pub fn has_rustup(&self) -> bool {
        self.contains().contains(MirrorContents::RUSTUP)
    }

    pub fn has_toolchain(&self) -> bool {
        self.contains().contains(MirrorContents::TOOLCHAIN)
    }

    pub fn has_components(&self) -> bool {
        self.contains().contains(MirrorContents::COMPONENT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contents_serialize_as_a_single_byte() {
        let contents = MirrorContents::TUF | MirrorContents::RUSTUP;
        assert_eq!(serde_json::to_string(&contents).unwrap(), "3");
        assert_eq!(
            serde_json::from_str::<MirrorContents>("3").unwrap(),
            contents
        );
    }

    #[test]
    fn unknown_content_bits_are_rejected() {
        assert!(serde_json::from_str::<MirrorContents>("128").is_err());
        assert!(serde_json::from_str::<MirrorContents>("256").is_err());
    }

    #[test]
    fn mirror_exposes_contents() {
        let mirror = Mirror::new(
            "https://a.example/".parse().unwrap(),
            MirrorContents::TOOLCHAIN,
        );
        assert_eq!(mirror.contains(), MirrorContents::TOOLCHAIN);
        assert!(mirror.has_toolchain());
        assert!(!mirror.has_tuf());
        assert!(!mirror.has_rustup());
        assert!(!mirror.has_components());
    }
}
