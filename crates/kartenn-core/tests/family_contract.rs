//! Contract battery shared by every address family (REQ-002-1): the same properties must
//! hold for IPv4, IPv6 and, with `--features test-family`, a fictitious 64-bit family.

use kartenn_core::addr::{Addr, Family, Prefix};
use proptest::prelude::*;

fn bits_of(family: Family) -> impl Strategy<Value = u128> {
    let width = family.width();
    any::<u128>().prop_map(move |b| {
        if width >= 128 {
            b
        } else {
            b & ((1u128 << width) - 1)
        }
    })
}

fn check_round_trip(family: Family, bits: u128) -> Result<(), TestCaseError> {
    let a = Addr::new(family, bits).expect("bits are masked to the width");
    let text = a.to_string();
    prop_assert_eq!(Addr::parse(family, &text), Ok(a), "round trip of {}", text);
    Ok(())
}

fn check_prefix(family: Family, bits: u128, len_seed: u8) -> Result<(), TestCaseError> {
    let width = family.width();
    let len = len_seed % (width + 1);
    let a = Addr::new(family, bits).expect("bits are masked to the width");
    let p = Prefix::containing(a, len).expect("len is within the width");
    prop_assert!(p.contains_addr(a));
    prop_assert!(p.network() <= a && a <= p.last());
    prop_assert_eq!(Prefix::new(p.network(), len), Ok(p));
    prop_assert_eq!(Prefix::parse(family, &p.to_string()), Ok(p));
    let host = width - len;
    prop_assert_eq!(p.size(), (host < 128).then(|| 1u128 << host));
    let up = p.supernet(len / 2).expect("shorter length");
    prop_assert!(up.contains(p));
    Ok(())
}

fn check_order(family: Family, x: u128, y: u128) -> Result<(), TestCaseError> {
    let a = Addr::new(family, x).expect("masked");
    let b = Addr::new(family, y).expect("masked");
    prop_assert_eq!(a.cmp(&b), x.cmp(&y));
    Ok(())
}

fn check_junk_refused(family: Family) {
    for junk in ["", " ", "%", "not-an-address", "1.2.3.4.5.6.7.8.9", "::::"] {
        assert!(
            Addr::parse(family, junk).is_err(),
            "{} accepted {junk:?}",
            family.id()
        );
    }
}

macro_rules! contract {
    ($name:ident, $family:expr) => {
        mod $name {
            use super::*;

            #[test]
            fn junk_is_refused() {
                check_junk_refused($family);
            }

            proptest! {
                #[test]
                fn round_trip(bits in bits_of($family)) {
                    check_round_trip($family, bits)?;
                }

                #[test]
                fn prefixes(bits in bits_of($family), len in any::<u8>()) {
                    check_prefix($family, bits, len)?;
                }

                #[test]
                fn ordering_follows_bits(x in bits_of($family), y in bits_of($family)) {
                    check_order($family, x, y)?;
                }
            }
        }
    };
}

contract!(ipv4, kartenn_core::family::IPV4);
contract!(ipv6, kartenn_core::family::IPV6);
#[cfg(feature = "test-family")]
contract!(octet8, kartenn_core::family::test64::OCTET8);
