pub mod client;
pub mod encoding;
pub mod resolver;
pub mod set;

use std::net::{Ipv6Addr, SocketAddr};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use simple_dns::{RCODE, SimpleDnsError};
use url::Url;

pub use client::MirrorSelectClient;
pub use encoding::MirrorEntry;
pub use resolver::SelectResolver;
pub use set::MirrorSet;

pub const DEFAULT_DNS_NAME: &str = "mirrors.rustup.rs";
pub const DNS_PORT: u16 = 53;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
pub const MAX_RESPONSE_LEN: usize = 4096;
pub const SITE_LOCAL_RESOLVERS: [Ipv6Addr; 3] = [
    Ipv6Addr::new(0xfec0, 0, 0, 0xffff, 0, 0, 0, 1),
    Ipv6Addr::new(0xfec0, 0, 0, 0xffff, 0, 0, 0, 2),
    Ipv6Addr::new(0xfec0, 0, 0, 0xffff, 0, 0, 0, 3),
];
#[cfg(windows)]
pub(crate) const ADAPTERS_BUFFER_BYTES: u32 = 15 * 1024;
#[cfg(windows)]
pub(crate) const ADAPTERS_MAX_ATTEMPTS: usize = 3;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("malformed DNS message")]
    Dns(#[from] SimpleDnsError),
    #[error("DNS transport failed")]
    Io(#[from] std::io::Error),
    #[error("no DNS servers are configured on this system")]
    NoNameservers,
    #[cfg(unix)]
    #[error("could not parse resolv.conf")]
    ResolvConf(#[from] resolv_conf::ParseError),
    #[error("no DNS response from {server} within {timeout:?}")]
    Timeout {
        server: SocketAddr,
        timeout: Duration,
    },
    #[error("DNS query for {name} answered with {rcode:?}")]
    Rcode { name: String, rcode: RCODE },
    #[error("TXT record is not valid UTF-8 text")]
    Text(#[from] std::string::FromUtf8Error),
    #[error("mirror entry is not valid base64")]
    Base64(#[from] base64::DecodeError),
    #[error("mirror entry does not describe a mirror")]
    Mirror(#[from] serde_json::Error),
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
