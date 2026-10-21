//! EUI-48 address of an interface (RFC 9542, 009/M7): read with `-`, `:` or in three
//! dot-separated groups of four digits, written as RFC 9542 §1.1 does, upper case with
//! hyphens. A group address (bit M of the first octet, RFC 9542 §2.1.1) is refused; a
//! local one (bit X) is accepted, as virtual machines use them.

use std::fmt;

use kartenn_core::rules::Violation;

use crate::rules::MAC_INVALID;

/// Group bit (M) of the first octet.
const GROUP_BIT: u8 = 0x01;
/// Local bit (X) of the first octet.
const LOCAL_BIT: u8 = 0x02;

/// A unicast EUI-48 address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mac([u8; 6]);

impl Mac {
    /// Parses an address written `00-00-5E-00-53-01`, `00:00:5e:00:53:01` or
    /// `0000.5e00.5301`, in either case.
    ///
    /// # Errors
    /// `machines.mac.invalid` for any other form or a group address.
    pub fn parse(input: &str) -> Result<Self, Violation> {
        let invalid = |why: &str| Violation::new(&MAC_INVALID, format!("MAC « {input} » : {why}"));
        let separator = ['-', ':', '.']
            .into_iter()
            .find(|&sep| input.contains(sep))
            .unwrap_or('-');
        let groups: Vec<&str> = input.split(separator).collect();
        let width = match (separator, groups.len()) {
            ('-' | ':', 6) => 2,
            ('.', 3) => 4,
            _ => {
                return Err(invalid(
                    "forme attendue xx-xx-xx-xx-xx-xx, xx:xx:xx:xx:xx:xx ou xxxx.xxxx.xxxx",
                ));
            }
        };
        let mut octets = [0_u8; 6];
        let mut at = 0;
        for group in groups {
            if group.len() != width || !group.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(invalid("chiffres hexadécimaux attendus"));
            }
            for pair in group.as_bytes().chunks(2) {
                let pair = std::str::from_utf8(pair)
                    .map_err(|_| invalid("chiffres hexadécimaux attendus"))?;
                octets[at] = u8::from_str_radix(pair, 16)
                    .map_err(|_| invalid("chiffres hexadécimaux attendus"))?;
                at += 1;
            }
        }
        if octets[0] & GROUP_BIT != 0 {
            return Err(invalid(
                "adresse de groupe (bit M à 1), une interface porte une adresse unicast",
            ));
        }
        Ok(Self(octets))
    }

    /// The six octets.
    #[must_use]
    pub fn octets(self) -> [u8; 6] {
        self.0
    }

    /// Whether the address is locally administered (bit X); a universal address is the
    /// one `machines.mac.duplicate` watches.
    #[must_use]
    pub fn is_local(self) -> bool {
        self.0[0] & LOCAL_BIT != 0
    }
}

impl fmt::Display for Mac {
    #[allow(clippy::many_single_char_names)] // the six octets, in order
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a, b, c, d, e, g] = self.0;
        write!(f, "{a:02X}-{b:02X}-{c:02X}-{d:02X}-{e:02X}-{g:02X}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_forms_and_canonical() {
        for form in [
            "00:00:5e:00:53:01",
            "0000.5e00.5301",
            "00-00-5E-00-53-01",
            "00-00-5e-00-53-01",
        ] {
            assert_eq!(
                Mac::parse(form).unwrap().to_string(),
                "00-00-5E-00-53-01",
                "{form}"
            );
        }
        let local = Mac::parse("02-00-5E-00-53-01").unwrap();
        assert!(local.is_local());
        assert!(!Mac::parse("00-00-5E-00-53-01").unwrap().is_local());
        for ko in [
            "",
            "00:00:5e:00:53",
            "00:00:5e:00:53:01:02",
            "00-00:5e-00:53-01",
            "0:0:5e:0:53:1",
            "0000.5e00.530",
            "00005e005301",
            "gg:00:5e:00:53:01",
            "+0:00:5e:00:53:01",
            "00.00.5e.00.53.01",
        ] {
            assert!(
                Mac::parse(ko).is_err_and(|v| v.rule.id == "machines.mac.invalid"),
                "{ko}"
            );
        }
    }

    #[test]
    fn mac_group_bit_refused() {
        for group in [
            "01-00-5E-00-00-01",
            "FF-FF-FF-FF-FF-FF",
            "33:33:00:00:00:01",
            "0300.0000.0001",
        ] {
            assert!(
                Mac::parse(group).is_err_and(|v| v.rule.id == "machines.mac.invalid"),
                "{group}"
            );
        }
    }
}
