mod common;

use std::time::Duration;

use common::{MAX_CHARACTER_STRING, MockDnsServer, mirrors};
use rust_mirror_select::{Error, MirrorSelectClient, MirrorSet};
use simple_dns::RCODE;

#[tokio::test]
async fn fetches_published_mirror_set() {
    let server = MockDnsServer::start().await;
    let expected = mirrors([
        "https://static.rust-lang.org/",
        "https://mirror.example/rustup/",
    ]);
    server.publish("mirrors.rustup.rs", &expected);

    let set = MirrorSet::with_dns_record(&server.client(), "mirrors.rustup.rs")
        .await
        .unwrap();

    assert_eq!(set, expected);
}

#[tokio::test]
async fn trailing_dot_and_case_do_not_matter() {
    let server = MockDnsServer::start().await;
    let expected = mirrors(["https://static.rust-lang.org/"]);
    server.publish("mirrors.rustup.rs", &expected);

    let set = MirrorSet::with_dns_record(&server.client(), "Mirrors.Rustup.RS.")
        .await
        .unwrap();

    assert_eq!(set, expected);
}

#[tokio::test]
async fn empty_record_yields_empty_set() {
    let server = MockDnsServer::start().await;
    server.publish("mirrors.rustup.rs", &MirrorSet::default());

    let set = MirrorSet::with_dns_record(&server.client(), "mirrors.rustup.rs")
        .await
        .unwrap();

    assert!(set.is_empty());
}

#[tokio::test]
async fn unknown_name_is_reported_as_nxdomain() {
    let server = MockDnsServer::start().await;

    let err = MirrorSet::with_dns_record(&server.client(), "missing.rustup.rs")
        .await
        .unwrap_err();

    assert!(
        matches!(err, Error::Rcode { ref name, rcode: RCODE::NameError } if name == "missing.rustup.rs"),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn long_entry_spanning_several_character_strings_is_reassembled() {
    let server = MockDnsServer::start().await;
    let long_path = "segment/".repeat(60);
    let expected = mirrors([format!("https://mirror.example/{long_path}").as_str()]);
    let entry = expected.to_entries().unwrap()[0].to_string();
    assert!(
        entry.len() > MAX_CHARACTER_STRING,
        "entry must need splitting"
    );
    server.publish("mirrors.rustup.rs", &expected);

    let set = MirrorSet::with_dns_record(&server.client(), "mirrors.rustup.rs")
        .await
        .unwrap();

    assert_eq!(set, expected);
}

#[tokio::test]
async fn malformed_entry_is_rejected() {
    let server = MockDnsServer::start().await;
    server.publish_raw("mirrors.rustup.rs", ["this is not base64"]);

    let err = MirrorSet::with_dns_record(&server.client(), "mirrors.rustup.rs")
        .await
        .unwrap_err();

    assert!(matches!(err, Error::Base64(_)), "unexpected error: {err:?}");
}

#[tokio::test]
async fn republishing_replaces_the_set() {
    let server = MockDnsServer::start().await;
    let client = server.client();

    server.publish("mirrors.rustup.rs", &mirrors(["https://old.example/"]));
    let first = MirrorSet::with_dns_record(&client, "mirrors.rustup.rs")
        .await
        .unwrap();
    assert_eq!(first, mirrors(["https://old.example/"]));

    server.publish("mirrors.rustup.rs", &mirrors(["https://new.example/"]));
    let second = MirrorSet::with_dns_record(&client, "mirrors.rustup.rs")
        .await
        .unwrap();
    assert_eq!(second, mirrors(["https://new.example/"]));

    server.remove("mirrors.rustup.rs");
    let err = MirrorSet::with_dns_record(&client, "mirrors.rustup.rs")
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        Error::Rcode {
            rcode: RCODE::NameError,
            ..
        }
    ));
}

#[tokio::test]
async fn unreachable_server_times_out() {
    let server = MockDnsServer::start().await;
    let addr = server.addr();
    drop(server);

    let client = MirrorSelectClient::with_timeout(addr, Duration::from_millis(200));
    let err = MirrorSet::with_dns_record(&client, "mirrors.rustup.rs")
        .await
        .unwrap_err();

    assert!(
        matches!(err, Error::Timeout { .. } | Error::Io(_)),
        "unexpected error: {err:?}"
    );
}
