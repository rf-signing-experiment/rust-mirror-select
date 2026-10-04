use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use crate::{DEFAULT_TIMEOUT, Error, MirrorSelectClient, SITE_LOCAL_RESOLVERS};

#[cfg(unix)]
pub use sys::parse_resolv_conf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectResolver {
    servers: Vec<SocketAddr>,
    timeout: Duration,
}

impl SelectResolver {
    pub fn new(servers: impl IntoIterator<Item = SocketAddr>) -> Self {
        let mut unique = Vec::new();
        for server in servers {
            if !unique.contains(&server) {
                unique.push(server);
            }
        }
        Self {
            servers: unique,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn system() -> Result<Self, Error> {
        let servers: Vec<_> = sys::nameservers()?
            .into_iter()
            .filter(|server| is_usable_nameserver(server.ip()))
            .collect();
        if servers.is_empty() {
            return Err(Error::NoNameservers);
        }
        Ok(Self::new(servers))
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn servers(&self) -> &[SocketAddr] {
        &self.servers
    }

    pub async fn lookup_txt(&self, name: &str) -> Result<Vec<String>, Error> {
        let mut last = Error::NoNameservers;
        for &server in &self.servers {
            let client = MirrorSelectClient::with_timeout(server, self.timeout);
            match client.lookup_txt(name).await {
                Err(err @ (Error::Io(_) | Error::Timeout { .. })) => last = err,
                result => return result,
            }
        }
        Err(last)
    }
}

pub fn is_usable_nameserver(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => !v4.is_unspecified(),
        IpAddr::V6(v6) => !v6.is_unspecified() && !SITE_LOCAL_RESOLVERS.contains(&v6),
    }
}

#[cfg(unix)]
mod sys {
    use std::io::ErrorKind;
    use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
    use std::path::PathBuf;
    use std::{env, fs};

    use resolv_conf::{Config, ScopedIp};

    use crate::{DNS_PORT, Error};

    pub fn nameservers() -> Result<Vec<SocketAddr>, Error> {
        let contents = candidate_paths()
            .find_map(|path| match fs::read(path) {
                Err(err) if err.kind() == ErrorKind::NotFound => None,
                result => Some(result),
            })
            .transpose()?
            .unwrap_or_default();
        parse_resolv_conf(&contents)
    }

    pub fn parse_resolv_conf(contents: &[u8]) -> Result<Vec<SocketAddr>, Error> {
        let config = Config::parse(contents)?;
        let nameservers = if cfg!(target_os = "android") {
            config.nameservers
        } else {
            config.get_nameservers_or_local()
        };
        Ok(nameservers.into_iter().map(to_socket_addr).collect())
    }

    fn candidate_paths() -> impl Iterator<Item = PathBuf> {
        let termux = cfg!(target_os = "android")
            .then(|| env::var_os("PREFIX"))
            .flatten()
            .map(|prefix| PathBuf::from(prefix).join("etc/resolv.conf"));
        let system = cfg!(target_os = "android").then(|| PathBuf::from("/system/etc/resolv.conf"));
        termux
            .into_iter()
            .chain([PathBuf::from("/etc/resolv.conf")])
            .chain(system)
    }

    fn to_socket_addr(ip: ScopedIp) -> SocketAddr {
        match ip {
            ScopedIp::V4(v4) => SocketAddr::V4(SocketAddrV4::new(v4, DNS_PORT)),
            ScopedIp::V6(v6, scope) => {
                let scope_id = scope.and_then(|s| s.parse().ok()).unwrap_or(0);
                SocketAddr::V6(SocketAddrV6::new(v6, DNS_PORT, 0, scope_id))
            }
        }
    }
}

#[cfg(windows)]
mod sys {
    use std::iter::successors;
    use std::mem::size_of;
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
    use std::ptr;

    use windows_sys::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_NO_DATA, NO_ERROR};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_FRIENDLY_NAME, GAA_FLAG_SKIP_MULTICAST,
        GAA_FLAG_SKIP_UNICAST, GetAdaptersAddresses, IP_ADAPTER_ADDRESSES_LH,
    };
    use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows_sys::Win32::Networking::WinSock::{
        AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6, SOCKET_ADDRESS,
    };

    use crate::{ADAPTERS_BUFFER_BYTES, ADAPTERS_MAX_ATTEMPTS, DNS_PORT, Error};

    pub fn nameservers() -> Result<Vec<SocketAddr>, Error> {
        let flags = GAA_FLAG_SKIP_UNICAST
            | GAA_FLAG_SKIP_ANYCAST
            | GAA_FLAG_SKIP_MULTICAST
            | GAA_FLAG_SKIP_FRIENDLY_NAME;
        let mut size = ADAPTERS_BUFFER_BYTES;
        let mut buffer: Vec<u64> = Vec::new();
        for _ in 0..ADAPTERS_MAX_ATTEMPTS {
            buffer.resize(size.div_ceil(size_of::<u64>() as u32) as usize, 0);
            let adapters = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
            let status = unsafe {
                GetAdaptersAddresses(
                    u32::from(AF_UNSPEC),
                    flags,
                    ptr::null(),
                    adapters,
                    &mut size,
                )
            };
            match status {
                NO_ERROR => return Ok(unsafe { collect(adapters) }),
                ERROR_NO_DATA => return Ok(Vec::new()),
                ERROR_BUFFER_OVERFLOW => {}
                code => return Err(std::io::Error::from_raw_os_error(code as i32).into()),
            }
        }
        Err(std::io::Error::from_raw_os_error(ERROR_BUFFER_OVERFLOW as i32).into())
    }

    unsafe fn linked_list<'a, T: 'a>(
        first: *mut T,
        next: impl Fn(&T) -> *mut T + 'a,
    ) -> impl Iterator<Item = &'a T> {
        successors(unsafe { first.as_ref() }, move |node| unsafe {
            next(node).as_ref()
        })
    }

    unsafe fn collect(adapters: *mut IP_ADAPTER_ADDRESSES_LH) -> Vec<SocketAddr> {
        unsafe { linked_list(adapters, |adapter| adapter.Next) }
            .filter(|adapter| adapter.OperStatus == IfOperStatusUp)
            .flat_map(|adapter| unsafe {
                linked_list(adapter.FirstDnsServerAddress, |dns| dns.Next)
            })
            .filter_map(|dns| unsafe { socket_addr(&dns.Address) })
            .collect()
    }

    unsafe fn socket_addr(address: &SOCKET_ADDRESS) -> Option<SocketAddr> {
        let sockaddr = address.lpSockaddr;
        if sockaddr.is_null() {
            return None;
        }
        let len = usize::try_from(address.iSockaddrLength).ok()?;
        let family = unsafe { ptr::addr_of!((*sockaddr).sa_family).read_unaligned() };
        match family {
            AF_INET if len >= size_of::<SOCKADDR_IN>() => {
                let v4 = unsafe { sockaddr.cast::<SOCKADDR_IN>().read_unaligned() };
                let octets = unsafe { v4.sin_addr.S_un.S_addr }.to_ne_bytes();
                Some(SocketAddr::V4(SocketAddrV4::new(
                    Ipv4Addr::from(octets),
                    DNS_PORT,
                )))
            }
            AF_INET6 if len >= size_of::<SOCKADDR_IN6>() => {
                let v6 = unsafe { sockaddr.cast::<SOCKADDR_IN6>().read_unaligned() };
                let (octets, scope_id) =
                    unsafe { (v6.sin6_addr.u.Byte, v6.Anonymous.sin6_scope_id) };
                Some(SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::from(octets),
                    DNS_PORT,
                    0,
                    scope_id,
                )))
            }
            _ => None,
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod sys {
    use std::net::SocketAddr;

    use crate::Error;

    pub fn nameservers() -> Result<Vec<SocketAddr>, Error> {
        Ok(Vec::new())
    }
}
