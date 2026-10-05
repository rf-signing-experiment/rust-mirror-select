use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use crate::common::{MockDnsServer, mirrors};
use rust_mirror_select::consts::DNS_PORT;
use rust_mirror_select::dns::resolver::is_usable_nameserver;
use rust_mirror_select::{Error, MirrorSet, SelectResolver};
use simple_dns::RCODE;

#[tokio::test]
async fn resolver_falls_through_to_the_next_server() {
    let dead = MockDnsServer::start().await;
    let dead_addr = dead.addr();
    drop(dead);

    let live = MockDnsServer::start().await;
    let expected = mirrors(["https://mirror.example/rustup/"]);
    live.publish("mirrors.rustup.rs", &expected);

    let resolver =
        SelectResolver::new([dead_addr, live.addr()]).with_timeout(Duration::from_millis(200));
    assert_eq!(resolver.servers(), [dead_addr, live.addr()]);

    let set = MirrorSet::with_resolver(&resolver, "mirrors.rustup.rs")
        .await
        .unwrap();
    assert_eq!(set, expected);
}

#[tokio::test]
async fn resolver_stops_at_a_definitive_answer() {
    let first = MockDnsServer::start().await;
    let second = MockDnsServer::start().await;
    second.publish("mirrors.rustup.rs", &mirrors(["https://mirror.example/"]));

    let resolver = SelectResolver::new([first.addr(), second.addr()]);
    let err = MirrorSet::with_resolver(&resolver, "mirrors.rustup.rs")
        .await
        .unwrap_err();

    assert!(
        matches!(
            err,
            Error::Rcode {
                rcode: RCODE::NameError,
                ..
            }
        ),
        "NXDOMAIN from the first server must not fall through: {err:?}"
    );
}

#[tokio::test]
async fn resolver_without_servers_reports_no_nameservers() {
    let resolver = SelectResolver::new([]);
    let err = resolver.lookup_txt("mirrors.rustup.rs").await.unwrap_err();
    assert!(
        matches!(err, Error::NoNameservers),
        "unexpected error: {err:?}"
    );
}

#[test]
fn system_resolver_discovers_at_least_one_server() {
    let resolver = SelectResolver::system().expect("system nameservers");
    assert!(!resolver.servers().is_empty());
    assert!(resolver.servers().iter().all(|s| s.port() == DNS_PORT));
}

#[test]
fn site_local_placeholders_are_rejected() {
    for last in 1..=3u16 {
        let ip = IpAddr::V6(Ipv6Addr::new(0xfec0, 0, 0, 0xffff, 0, 0, 0, last));
        assert!(!is_usable_nameserver(ip));
    }
    assert!(!is_usable_nameserver("0.0.0.0".parse().unwrap()));
    assert!(!is_usable_nameserver("::".parse().unwrap()));
    assert!(is_usable_nameserver("192.0.2.1".parse().unwrap()));
    assert!(is_usable_nameserver("2001:db8::53".parse().unwrap()));
    assert!(is_usable_nameserver("fec0:0:0:ffff::4".parse().unwrap()));
}

#[test]
fn new_dedups_preserving_order() {
    let a: SocketAddr = "192.0.2.1:53".parse().unwrap();
    let b: SocketAddr = "192.0.2.2:53".parse().unwrap();
    let resolver = SelectResolver::new([a, b, a]);
    assert_eq!(resolver.servers(), [a, b]);
}

#[cfg(unix)]
mod resolv_conf {
    use std::net::SocketAddr;

    use rust_mirror_select::dns::resolver::parse_resolv_conf;

    #[test]
    fn parses_nameservers_in_order() {
        let conf = b"# comment\nsearch example.org\nnameserver 192.0.2.1\nnameserver 2001:db8::53\nnameserver 2001:db8::1%7\noptions ndots:2\n";
        let servers = parse_resolv_conf(conf).unwrap();
        assert_eq!(
            servers,
            [
                "192.0.2.1:53".parse::<SocketAddr>().unwrap(),
                "[2001:db8::53]:53".parse().unwrap(),
                "[2001:db8::1%7]:53".parse().unwrap(),
            ]
        );
    }

    #[cfg(not(target_os = "android"))]
    #[test]
    fn empty_config_means_loopback() {
        let servers = parse_resolv_conf(b"search example.org\n").unwrap();
        assert!(!servers.is_empty());
        assert!(servers.iter().all(|s| s.ip().is_loopback()));
    }

    #[cfg(target_os = "android")]
    #[test]
    fn empty_config_means_nothing_on_android() {
        assert!(parse_resolv_conf(b"").unwrap().is_empty());
    }
}
