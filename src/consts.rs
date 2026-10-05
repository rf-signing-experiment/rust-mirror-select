#[cfg(feature = "dns")]
use std::net::Ipv6Addr;
#[cfg(any(feature = "dns", feature = "ping-select", feature = "speedtest-select"))]
use std::time::Duration;

#[cfg(feature = "dns")]
pub const DEFAULT_DNS_NAME: &str = "mirrors.rustup.rs";
#[cfg(feature = "dns")]
pub const DNS_PORT: u16 = 53;
#[cfg(feature = "dns")]
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(feature = "dns")]
pub const MAX_RESPONSE_LEN: usize = 4096;
#[cfg(feature = "dns")]
pub const SITE_LOCAL_RESOLVERS: [Ipv6Addr; 3] = [
    Ipv6Addr::new(0xfec0, 0, 0, 0xffff, 0, 0, 0, 1),
    Ipv6Addr::new(0xfec0, 0, 0, 0xffff, 0, 0, 0, 2),
    Ipv6Addr::new(0xfec0, 0, 0, 0xffff, 0, 0, 0, 3),
];
#[cfg(all(feature = "dns", windows))]
pub(crate) const ADAPTERS_BUFFER_BYTES: u32 = 15 * 1024;
#[cfg(all(feature = "dns", windows))]
pub(crate) const ADAPTERS_MAX_ATTEMPTS: usize = 3;

#[cfg(feature = "ping-select")]
pub const PING_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(feature = "speedtest-select")]
pub const SPEEDTEST_PATH: &str = "dummy";
#[cfg(feature = "speedtest-select")]
pub const SPEEDTEST_SIZE: u64 = 10 * 1024 * 1024;
#[cfg(feature = "speedtest-select")]
pub const SPEEDTEST_TIMEOUT: Duration = Duration::from_secs(30);
