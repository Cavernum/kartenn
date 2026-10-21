//! Prefix and address parsing through the public API (REQ-002-4, REQ-002-5).

use kartenn_core::addr::{Addr, AddrError, Prefix};
use kartenn_core::family::{ALL, IPV4, IPV6};

#[test]
fn zones_are_refused_with_a_dedicated_message() {
    for text in ["fe80::1%eth0", "fe80::1%2"] {
        let err = Addr::parse(IPV6, text).unwrap_err();
        assert!(matches!(err, AddrError::Zone { .. }), "{text}");
        assert!(err.to_string().contains("RFC 4007"));
    }
    assert!(matches!(
        Prefix::parse(IPV6, "fe80::%2/64"),
        Err(AddrError::Zone { .. })
    ));
    assert!(matches!(
        Addr::parse_any(&ALL, "fe80::1%eth0"),
        Err(AddrError::Zone { .. })
    ));
}

#[test]
fn aligned_prefixes_are_accepted() {
    assert_eq!(
        Prefix::parse(IPV4, "192.168.1.0/24").unwrap().to_string(),
        "192.168.1.0/24"
    );
    assert_eq!(
        Prefix::parse(IPV6, "2001:DB8::/32").unwrap().to_string(),
        "2001:db8::/32"
    );
    assert_eq!(Prefix::parse(IPV4, "0.0.0.0/0").unwrap().prefix_len(), 0);
    assert_eq!(Prefix::parse(IPV6, "::/0").unwrap().size(), None);
}

#[test]
fn host_bits_are_refused_with_the_aligned_suggestion() {
    let err = Prefix::parse(IPV4, "192.168.1.5/24").unwrap_err();
    let AddrError::HostBitsSet { aligned, .. } = &err else {
        panic!("unexpected error {err:?}");
    };
    assert_eq!(aligned.to_string(), "192.168.1.0/24");
    assert!(err.to_string().contains("did you mean 192.168.1.0/24"));
}

#[test]
fn malformed_lengths_and_implicit_lengths_are_refused() {
    for text in [
        "10",
        "10/8",
        "10.0.0.0",
        "10.0.0.0/",
        "10.0.0.0/08",
        "10.0.0.0/+8",
        "10.0.0.0/ 8",
        "10.0.0.0/8 ",
    ] {
        assert!(
            Prefix::parse(IPV4, text).is_err(),
            "{text:?} must be refused"
        );
    }
    assert!(matches!(
        Prefix::parse(IPV4, "10.0.0.0/33"),
        Err(AddrError::LengthOutOfRange {
            len: 33,
            width: 32,
            ..
        })
    ));
    assert!(matches!(
        Prefix::parse(IPV6, "2001:db8::/129"),
        Err(AddrError::LengthOutOfRange {
            len: 129,
            width: 128,
            ..
        })
    ));
    assert!(matches!(
        Prefix::parse(IPV4, "10.0.0.0/999"),
        Err(AddrError::PrefixSyntax { .. })
    ));
}

#[test]
fn the_family_is_detected_from_the_text() {
    assert_eq!(Addr::parse_any(&ALL, "192.0.2.1").unwrap().family(), IPV4);
    assert_eq!(Addr::parse_any(&ALL, "2001:db8::1").unwrap().family(), IPV6);
    assert_eq!(
        Prefix::parse_any(&ALL, "2001:db8::/48").unwrap().family(),
        IPV6
    );
    assert!(matches!(
        Addr::parse_any(&ALL, "srv.example.org"),
        Err(AddrError::UnknownFamily { .. })
    ));
}
