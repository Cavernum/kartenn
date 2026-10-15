//! The IPv4 and IPv6 address families (REQ-002-2, 3, 6, 7).
//!
//! Parsing and canonical output delegate to `std::net`, whose behaviour matches the
//! strict rules of spec 002 (verified on 2026-10-14 with rustc 1.98.1, see
//! `specs/002-noyau-adressage/design.md`). The tests below pin that behaviour: a change
//! in the standard library would change our validation rules and must be noticed.

use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};

use crate::addr::{AddrError, AddressFamily, Family};

/// IPv4 (RFC 791): strict dotted decimal only (RFC 6943 §3.1.1).
#[derive(Debug)]
pub struct Ipv4;

/// IPv6 (RFC 4291): every legitimate RFC 4291 form in, RFC 5952 form out.
#[derive(Debug)]
pub struct Ipv6;

static IPV4_IMPL: Ipv4 = Ipv4;
static IPV6_IMPL: Ipv6 = Ipv6;

/// Handle on the IPv4 family.
pub const IPV4: Family = Family::new(&IPV4_IMPL);
/// Handle on the IPv6 family.
pub const IPV6: Family = Family::new(&IPV6_IMPL);

impl AddressFamily for Ipv4 {
    fn id(&self) -> &'static str {
        "ipv4"
    }

    fn width(&self) -> u8 {
        32
    }

    fn parse(&self, text: &str) -> Result<u128, AddrError> {
        text.parse::<Ipv4Addr>().map(|a| u128::from(a.to_bits())).map_err(|_| AddrError::Syntax {
            family: "ipv4",
            text: text.to_owned(),
            expected: "four decimal numbers from 0 to 255 separated by dots, without leading zeros",
        })
    }

    fn write_canonical(&self, bits: u128, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bits = u32::try_from(bits).map_err(|_| fmt::Error)?;
        write!(f, "{}", Ipv4Addr::from_bits(bits))
    }
}

impl AddressFamily for Ipv6 {
    fn id(&self) -> &'static str {
        "ipv6"
    }

    fn width(&self) -> u8 {
        128
    }

    fn parse(&self, text: &str) -> Result<u128, AddrError> {
        text.parse::<Ipv6Addr>()
            .map(Ipv6Addr::to_bits)
            .map_err(|_| AddrError::Syntax {
                family: "ipv6",
                text: text.to_owned(),
                expected: "an IPv6 address in an RFC 4291 textual form",
            })
    }

    fn write_canonical(&self, bits: u128, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Ipv6Addr::from_bits(bits))
    }
}

/// Families compiled into this build, narrowest first.
pub const ALL: [Family; 2] = [IPV4, IPV6];

#[cfg(feature = "test-family")]
pub mod test64 {
    //! A fictitious 64-bit family written as eight dotted octets, compiled only with the
    //! `test-family` feature (REQ-002-1).
    use std::fmt;

    use crate::addr::{AddrError, AddressFamily, Family};

    /// Eight dotted decimal octets, without leading zeros.
    #[derive(Debug)]
    pub struct Octet8;

    static OCTET8_IMPL: Octet8 = Octet8;

    /// Handle on the fictitious 64-bit family.
    pub const OCTET8: Family = Family::new(&OCTET8_IMPL);

    fn syntax(text: &str) -> AddrError {
        AddrError::Syntax {
            family: "test-octet8",
            text: text.to_owned(),
            expected: "eight decimal numbers from 0 to 255 separated by dots",
        }
    }

    impl AddressFamily for Octet8 {
        fn id(&self) -> &'static str {
            "test-octet8"
        }

        fn width(&self) -> u8 {
            64
        }

        fn parse(&self, text: &str) -> Result<u128, AddrError> {
            let parts: Vec<&str> = text.split('.').collect();
            if parts.len() != 8 {
                return Err(syntax(text));
            }
            parts.iter().try_fold(0u128, |acc, part| {
                let well_formed = !part.is_empty()
                    && part.bytes().all(|b| b.is_ascii_digit())
                    && (*part == "0" || !part.starts_with('0'));
                let octet: u8 = if well_formed {
                    part.parse().map_err(|_| syntax(text))?
                } else {
                    return Err(syntax(text));
                };
                Ok((acc << 8) | u128::from(octet))
            })
        }

        fn write_canonical(&self, bits: u128, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let bytes = u64::try_from(bits).map_err(|_| fmt::Error)?.to_be_bytes();
            let text: Vec<String> = bytes.iter().map(u8::to_string).collect();
            f.write_str(&text.join("."))
        }
    }
}

/// Blocks whose addresses are never assigned to a host, whatever the network
/// (REQ-002-29): unspecified, loopback, "this network", reserved, limited broadcast and
/// multicast. Network, broadcast and zero-IID addresses depend on the network and are
/// checked separately.
#[must_use]
pub fn intrinsically_unassignable(family: Family) -> &'static [crate::addr::Prefix] {
    use std::sync::LazyLock;
    static V4: LazyLock<Vec<crate::addr::Prefix>> = LazyLock::new(|| {
        ["0.0.0.0/8", "127.0.0.0/8", "224.0.0.0/4", "240.0.0.0/4"]
            .iter()
            .map(|p| crate::addr::Prefix::parse(IPV4, p).expect("literal IPv4 prefix"))
            .collect()
    });
    static V6: LazyLock<Vec<crate::addr::Prefix>> = LazyLock::new(|| {
        ["::/128", "::1/128", "ff00::/8"]
            .iter()
            .map(|p| crate::addr::Prefix::parse(IPV6, p).expect("literal IPv6 prefix"))
            .collect()
    });
    if family == IPV4 {
        &V4
    } else if family == IPV6 {
        &V6
    } else {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addr::Addr;
    use proptest::prelude::*;

    #[test]
    fn ipv4_strict_parsing_is_pinned() {
        // REQ-002-2 acceptance cases and the cases verified against std::net.
        assert_eq!(
            Addr::parse(IPV4, "192.0.2.1").unwrap().to_string(),
            "192.0.2.1"
        );
        for bad in [
            "010.0.0.1",
            "01.2.3.4",
            "1.2.3.04",
            "10.1",
            "0x0a.0.0.1",
            "10.0.258",
            "256.0.0.1",
            " 1.2.3.4",
            "192.0.2.1 ",
            "",
        ] {
            let err = Addr::parse(IPV4, bad).unwrap_err();
            assert!(
                matches!(err, AddrError::Syntax { family: "ipv4", .. }),
                "{bad:?} must be refused"
            );
            assert!(
                err.to_string().contains("without leading zeros"),
                "message cites the expected form"
            );
        }
    }

    #[test]
    fn ipv6_accepts_rfc4291_forms_and_refuses_the_rest() {
        // REQ-002-3: all legitimate forms in (RFC 5952 §4).
        for (text, canonical) in [
            ("2001:DB8::1", "2001:db8::1"),
            ("2001:0db8:0000:0000:0000:0000:0000:0001", "2001:db8::1"),
            ("0:0:0:0:0:0:0:1", "::1"),
            ("::ffff:192.0.2.1", "::ffff:192.0.2.1"),
        ] {
            assert_eq!(
                Addr::parse(IPV6, text).unwrap().to_string(),
                canonical,
                "{text}"
            );
        }
        for bad in [
            "2001:db8::1::2",
            "2001:db8:::1",
            "12345::1",
            "::ffff:010.0.0.1",
            "fe80::1%eth0",
            "fe80::1%2",
            "",
        ] {
            assert!(Addr::parse(IPV6, bad).is_err(), "{bad:?} must be refused");
        }
    }

    #[test]
    fn ipv6_canonical_output_follows_rfc5952() {
        // REQ-002-6 acceptance: longest zero run compressed, a single zero group is not,
        // lowercase hexadecimal.
        let a = Addr::parse(IPV6, "2001:0DB8:0:0:1:0:0:1").unwrap();
        assert_eq!(a.to_string(), "2001:db8::1:0:0:1");
        let b = Addr::parse(IPV6, "2001:db8:0:1:1:1:1:1").unwrap();
        assert_eq!(b.to_string(), "2001:db8:0:1:1:1:1:1");
        // Longest run wins over the first, shorter one (RFC 5952 §4.2.3).
        let c = Addr::parse(IPV6, "2001:db8:0:0:1:0:0:0").unwrap();
        assert_eq!(c.to_string(), "2001:db8:0:0:1::");
    }

    #[test]
    fn equality_is_on_value_not_text() {
        // REQ-002-7.
        assert_eq!(
            Addr::parse(IPV6, "2001:DB8::").unwrap(),
            Addr::parse(IPV6, "2001:0db8::").unwrap()
        );
        assert_ne!(
            Addr::parse(IPV4, "0.0.0.1").unwrap(),
            Addr::parse(IPV6, "::1").unwrap()
        );
    }

    #[test]
    fn ipv4_sorts_before_ipv6() {
        assert!(Addr::parse(IPV4, "255.255.255.255").unwrap() < Addr::parse(IPV6, "::").unwrap());
    }

    proptest! {
        #[test]
        fn ipv4_round_trip(bits in any::<u32>()) {
            let a = Addr::new(IPV4, u128::from(bits)).unwrap();
            prop_assert_eq!(Addr::parse(IPV4, &a.to_string()), Ok(a));
        }

        #[test]
        fn ipv6_round_trip(bits in any::<u128>()) {
            let a = Addr::new(IPV6, bits).unwrap();
            let text = a.to_string();
            prop_assert_eq!(Addr::parse(IPV6, &text), Ok(a));
            prop_assert_eq!(text.to_lowercase(), text);
        }
    }
}
