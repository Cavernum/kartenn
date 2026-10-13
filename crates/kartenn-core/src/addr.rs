//! Address families, addresses and CIDR prefixes (spec 002, REQ-002-1).
//!
//! Every value is stored as `(family, bits: u128)`, the bits right-aligned within the
//! family width. Nothing outside the [`AddressFamily`] implementations assumes a width
//! of 32 or 128 bits: any family up to 128 bits fits (a 64-bit family is exercised in
//! the tests).

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

/// Contract of an address family (002/design, § Familles d'adresses).
///
/// Implementations are `'static` values compiled into the binary; a value refers to its
/// family through a [`Family`] handle.
pub trait AddressFamily: fmt::Debug + Sync + 'static {
    /// Stable identifier of the family (`ipv4`, `ipv6`…).
    fn id(&self) -> &'static str;

    /// Width of an address in bits, between 1 and 128.
    fn width(&self) -> u8;

    /// Parses the strict textual form of an address into its bits.
    ///
    /// # Errors
    ///
    /// Returns [`AddrError::Syntax`] when the text is not an address of this family.
    fn parse(&self, text: &str) -> Result<u128, AddrError>;

    /// Writes the canonical textual form of an address.
    ///
    /// # Errors
    ///
    /// Returns any error of the formatter.
    fn write_canonical(&self, bits: u128, f: &mut fmt::Formatter<'_>) -> fmt::Result;
}

/// Handle on a compiled address family. Two handles are equal when their identifiers are.
#[derive(Clone, Copy)]
pub struct Family(&'static dyn AddressFamily);

impl Family {
    /// Wraps a compiled family.
    #[must_use]
    pub const fn new(family: &'static dyn AddressFamily) -> Self {
        Self(family)
    }

    /// Stable identifier.
    #[must_use]
    pub fn id(self) -> &'static str {
        self.0.id()
    }

    /// Width in bits.
    #[must_use]
    pub fn width(self) -> u8 {
        self.0.width()
    }

    /// The family implementation.
    #[must_use]
    pub fn implementation(self) -> &'static dyn AddressFamily {
        self.0
    }

    /// Mask of all the bits of an address of this family.
    #[must_use]
    pub fn all_bits(self) -> u128 {
        ones(self.width())
    }
}

impl fmt::Debug for Family {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

impl PartialEq for Family {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Eq for Family {}

impl Hash for Family {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}

impl PartialOrd for Family {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Family {
    /// Narrower families first (IPv4 before IPv6, as PostgreSQL sorts `inet`), then by id.
    fn cmp(&self, other: &Self) -> Ordering {
        self.width()
            .cmp(&other.width())
            .then_with(|| self.id().cmp(other.id()))
    }
}

/// Errors on address and prefix values.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AddrError {
    /// The text is not a valid address of the family.
    #[error("{text:?} is not a valid {family} address: {expected}")]
    Syntax {
        /// Family that was expected.
        family: &'static str,
        /// Text that was given.
        text: String,
        /// Description of the expected form.
        expected: &'static str,
    },
    /// The bits do not fit in the family width.
    #[error("value out of range for a {family} address")]
    OutOfRange {
        /// Family of the value.
        family: &'static str,
    },
    /// The prefix length is larger than the family width.
    #[error("prefix length {len} is larger than {width} for {family}")]
    LengthOutOfRange {
        /// Family of the prefix.
        family: &'static str,
        /// Given length.
        len: u8,
        /// Width of the family.
        width: u8,
    },
    /// The prefix has host bits set; `aligned` is the prefix with those bits cleared.
    #[error("{given} has host bits set; did you mean {aligned}?")]
    HostBitsSet {
        /// Prefix as given.
        given: String,
        /// Aligned prefix suggested instead.
        aligned: Prefix,
    },
    /// The text carries a zone index (`%…`), meaningful only on one node (RFC 4007 §6).
    #[error(
        "{text:?} carries a zone index (%…), which is only meaningful on one node (RFC 4007 §6); remove it"
    )]
    Zone {
        /// Text that was given.
        text: String,
    },
    /// The text is not a prefix in `address/length` form.
    #[error("{text:?} is not a prefix: {reason}")]
    PrefixSyntax {
        /// Text that was given.
        text: String,
        /// What is wrong.
        reason: &'static str,
    },
    /// The text is not an address of any compiled family.
    #[error("{text:?} is not an address of any supported family")]
    UnknownFamily {
        /// Text that was given.
        text: String,
    },
    /// Two values of different families were combined.
    #[error("{left} and {right} are not of the same family")]
    FamilyMismatch {
        /// Family of the first value.
        left: &'static str,
        /// Family of the second value.
        right: &'static str,
    },
}

/// `n` low bits set (`n` up to 128).
const fn ones(n: u8) -> u128 {
    if n >= 128 {
        u128::MAX
    } else {
        (1u128 << n) - 1
    }
}

/// Mask of the `len` leading bits of a `width`-bit value.
const fn network_mask(width: u8, len: u8) -> u128 {
    ones(width) & !ones(width - len)
}

/// An address of a family.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Addr {
    family: Family,
    bits: u128,
}

impl Addr {
    /// Builds an address from its bits.
    ///
    /// # Errors
    ///
    /// [`AddrError::OutOfRange`] if `bits` does not fit in the family width.
    pub fn new(family: Family, bits: u128) -> Result<Self, AddrError> {
        if bits & !family.all_bits() != 0 {
            return Err(AddrError::OutOfRange {
                family: family.id(),
            });
        }
        Ok(Self { family, bits })
    }

    /// Parses the strict textual form of an address of `family`.
    ///
    /// # Errors
    ///
    /// The family's parse error.
    pub fn parse(family: Family, text: &str) -> Result<Self, AddrError> {
        refuse_zone(text)?;
        let bits = family.implementation().parse(text)?;
        Self::new(family, bits)
    }

    /// Parses an address of whichever family among `families` accepts the text. Families
    /// have disjoint textual forms, so at most one accepts it.
    ///
    /// # Errors
    ///
    /// [`AddrError::Zone`], or [`AddrError::UnknownFamily`] when no family accepts it.
    pub fn parse_any(families: &[Family], text: &str) -> Result<Self, AddrError> {
        refuse_zone(text)?;
        families
            .iter()
            .find_map(|&family| Self::parse(family, text).ok())
            .ok_or_else(|| AddrError::UnknownFamily {
                text: text.to_owned(),
            })
    }

    /// Family of the address.
    #[must_use]
    pub const fn family(self) -> Family {
        self.family
    }

    /// Bits of the address.
    #[must_use]
    pub const fn bits(self) -> u128 {
        self.bits
    }

    /// The address `offset` positions after this one, if it stays in the family.
    #[must_use]
    pub fn checked_add(self, offset: u128) -> Option<Self> {
        let bits = self.bits.checked_add(offset)?;
        Self::new(self.family, bits).ok()
    }

    /// The address `offset` positions before this one, if it stays in the family.
    #[must_use]
    pub fn checked_sub(self, offset: u128) -> Option<Self> {
        Some(Self {
            family: self.family,
            bits: self.bits.checked_sub(offset)?,
        })
    }

    /// Distance from `base` to this address, if `base` is not after it.
    ///
    /// # Errors
    ///
    /// [`AddrError::FamilyMismatch`] if the families differ.
    pub fn offset_from(self, base: Self) -> Result<Option<u128>, AddrError> {
        same_family(self.family, base.family)?;
        Ok(self.bits.checked_sub(base.bits))
    }
}

fn refuse_zone(text: &str) -> Result<(), AddrError> {
    if text.contains('%') {
        return Err(AddrError::Zone {
            text: text.to_owned(),
        });
    }
    Ok(())
}

/// Parses a prefix length: decimal digits only, no sign, no leading zero, no spaces.
fn parse_prefix_len(text: &str, digits: &str) -> Result<u8, AddrError> {
    let well_formed = !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'));
    if !well_formed {
        return Err(AddrError::PrefixSyntax {
            text: text.to_owned(),
            reason: "the length must be a decimal number without leading zeros",
        });
    }
    digits.parse().map_err(|_| AddrError::PrefixSyntax {
        text: text.to_owned(),
        reason: "the length is too large",
    })
}

fn same_family(left: Family, right: Family) -> Result<(), AddrError> {
    if left == right {
        Ok(())
    } else {
        Err(AddrError::FamilyMismatch {
            left: left.id(),
            right: right.id(),
        })
    }
}

impl PartialOrd for Addr {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Addr {
    fn cmp(&self, other: &Self) -> Ordering {
        self.family
            .cmp(&other.family)
            .then(self.bits.cmp(&other.bits))
    }
}

impl fmt::Display for Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.family.implementation().write_canonical(self.bits, f)
    }
}

impl fmt::Debug for Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

/// A CIDR prefix: an aligned network address and a length (RFC 4632 §3.1).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Prefix {
    network: Addr,
    len: u8,
}

impl Prefix {
    /// Builds a prefix, refusing host bits (REQ-002-5).
    ///
    /// # Errors
    ///
    /// [`AddrError::LengthOutOfRange`] or [`AddrError::HostBitsSet`] (with the aligned
    /// prefix as a suggestion).
    pub fn new(addr: Addr, len: u8) -> Result<Self, AddrError> {
        let aligned = Self::containing(addr, len)?;
        if aligned.network != addr {
            return Err(AddrError::HostBitsSet {
                given: format!("{addr}/{len}"),
                aligned,
            });
        }
        Ok(aligned)
    }

    /// Parses `address/length` strictly (REQ-002-4, 5): explicit length, no host bits, no
    /// zone, no implicit classful length.
    ///
    /// # Errors
    ///
    /// [`AddrError::Zone`], [`AddrError::PrefixSyntax`], the family's syntax error,
    /// [`AddrError::LengthOutOfRange`] or [`AddrError::HostBitsSet`] with a suggestion.
    pub fn parse(family: Family, text: &str) -> Result<Self, AddrError> {
        let (addr, len) = Self::split(text)?;
        Self::new(Addr::parse(family, addr)?, parse_prefix_len(text, len)?)
    }

    /// Parses a prefix of whichever family among `families` accepts its address part.
    ///
    /// # Errors
    ///
    /// As [`Prefix::parse`], or [`AddrError::UnknownFamily`].
    pub fn parse_any(families: &[Family], text: &str) -> Result<Self, AddrError> {
        let (addr, len) = Self::split(text)?;
        Self::new(
            Addr::parse_any(families, addr)?,
            parse_prefix_len(text, len)?,
        )
    }

    fn split(text: &str) -> Result<(&str, &str), AddrError> {
        refuse_zone(text)?;
        text.split_once('/').ok_or_else(|| AddrError::PrefixSyntax {
            text: text.to_owned(),
            reason: "expected address/length, the length is mandatory",
        })
    }

    /// The prefix of length `len` that contains `addr` (host bits cleared).
    ///
    /// # Errors
    ///
    /// [`AddrError::LengthOutOfRange`] if `len` exceeds the family width.
    pub fn containing(addr: Addr, len: u8) -> Result<Self, AddrError> {
        let width = addr.family.width();
        if len > width {
            return Err(AddrError::LengthOutOfRange {
                family: addr.family.id(),
                len,
                width,
            });
        }
        let bits = addr.bits & network_mask(width, len);
        Ok(Self {
            network: Addr {
                family: addr.family,
                bits,
            },
            len,
        })
    }

    /// Family of the prefix.
    #[must_use]
    pub const fn family(self) -> Family {
        self.network.family
    }

    /// Network address (first address).
    #[must_use]
    pub const fn network(self) -> Addr {
        self.network
    }

    /// Prefix length.
    #[must_use]
    pub const fn prefix_len(self) -> u8 {
        self.len
    }

    /// Number of host bits.
    #[must_use]
    pub fn host_bits(self) -> u8 {
        self.family().width() - self.len
    }

    /// Last address of the prefix.
    #[must_use]
    pub fn last(self) -> Addr {
        Addr {
            family: self.family(),
            bits: self.network.bits | ones(self.host_bits()),
        }
    }

    /// Number of addresses, or `None` when it is 2¹²⁸ (an IPv6 `/0`), which `u128`
    /// cannot represent.
    #[must_use]
    pub fn size(self) -> Option<u128> {
        let host = self.host_bits();
        (host < 128).then(|| 1u128 << host)
    }

    /// Whether `addr` belongs to the prefix.
    #[must_use]
    pub fn contains_addr(self, addr: Addr) -> bool {
        addr.family == self.family()
            && addr.bits & network_mask(self.family().width(), self.len) == self.network.bits
    }

    /// Whether `other` is inside this prefix or equal to it (`>>=`).
    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        other.len >= self.len && self.contains_addr(other.network)
    }

    /// Whether `other` is inside this prefix and different from it (`>>`).
    #[must_use]
    pub fn strictly_contains(self, other: Self) -> bool {
        other.len > self.len && self.contains_addr(other.network)
    }

    /// Whether the two prefixes share an address. Two CIDR blocks either nest or are
    /// disjoint (D34).
    #[must_use]
    pub fn overlaps(self, other: Self) -> bool {
        self.contains(other) || other.contains(self)
    }

    /// The enclosing prefix of length `len`, if `len` is not longer than this prefix.
    #[must_use]
    pub fn supernet(self, len: u8) -> Option<Self> {
        if len > self.len {
            return None;
        }
        let bits = self.network.bits & network_mask(self.family().width(), len);
        Some(Self {
            network: Addr {
                family: self.family(),
                bits,
            },
            len,
        })
    }
}

impl PartialOrd for Prefix {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Prefix {
    /// By network address, then shorter prefixes first: a prefix sorts before the prefixes
    /// it contains.
    fn cmp(&self, other: &Self) -> Ordering {
        self.network
            .cmp(&other.network)
            .then(self.len.cmp(&other.len))
    }
}

impl fmt::Display for Prefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.network, self.len)
    }
}

impl fmt::Debug for Prefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

#[cfg(test)]
pub(crate) mod test_family {
    //! A fictitious 8-bit family, written in decimal, for unit tests of the generic code.
    use super::{AddrError, AddressFamily, Family};
    use std::fmt;

    #[derive(Debug)]
    pub(crate) struct Byte;

    impl AddressFamily for Byte {
        fn id(&self) -> &'static str {
            "byte"
        }
        fn width(&self) -> u8 {
            8
        }
        fn parse(&self, text: &str) -> Result<u128, AddrError> {
            text.parse::<u8>()
                .map(u128::from)
                .map_err(|_| AddrError::Syntax {
                    family: "byte",
                    text: text.to_owned(),
                    expected: "a decimal number from 0 to 255",
                })
        }
        fn write_canonical(&self, bits: u128, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{bits}")
        }
    }

    pub(crate) static BYTE: Byte = Byte;
    pub(crate) const FAMILY: Family = Family::new(&BYTE);
}

#[cfg(test)]
mod tests {
    use super::test_family::FAMILY;
    use super::*;
    use proptest::prelude::*;

    fn addr(bits: u128) -> Addr {
        Addr::new(FAMILY, bits).unwrap()
    }

    fn prefix(bits: u128, len: u8) -> Prefix {
        Prefix::new(addr(bits), len).unwrap()
    }

    #[test]
    fn out_of_range_bits_are_refused() {
        assert_eq!(
            Addr::new(FAMILY, 256),
            Err(AddrError::OutOfRange { family: "byte" })
        );
        assert!(Addr::new(FAMILY, 255).is_ok());
    }

    #[test]
    fn host_bits_are_refused_with_a_suggestion() {
        let err = Prefix::new(addr(0b1010_0101), 4).unwrap_err();
        assert_eq!(
            err,
            AddrError::HostBitsSet {
                given: "165/4".into(),
                aligned: prefix(0b1010_0000, 4)
            }
        );
        assert_eq!(
            err.to_string(),
            "165/4 has host bits set; did you mean 160/4?"
        );
    }

    #[test]
    fn length_beyond_width_is_refused() {
        assert!(matches!(
            Prefix::new(addr(0), 9),
            Err(AddrError::LengthOutOfRange {
                len: 9,
                width: 8,
                ..
            })
        ));
    }

    #[test]
    fn bounds_and_size() {
        let p = prefix(0b1010_0000, 4);
        assert_eq!(p.last(), addr(0b1010_1111));
        assert_eq!(p.size(), Some(16));
        assert_eq!(prefix(0, 0).size(), Some(256));
        assert_eq!(prefix(7, 8).size(), Some(1));
    }

    #[test]
    fn containment_relations() {
        let outer = prefix(0b1010_0000, 4);
        let inner = prefix(0b1010_1000, 5);
        assert!(outer.contains(inner) && outer.strictly_contains(inner));
        assert!(outer.contains(outer) && !outer.strictly_contains(outer));
        assert!(!inner.contains(outer));
        assert!(outer.overlaps(inner) && inner.overlaps(outer));
        assert!(!prefix(0, 4).overlaps(prefix(16, 4)));
    }

    #[test]
    fn prefix_sorts_before_its_content() {
        let mut v = vec![
            prefix(0b1010_1000, 5),
            prefix(0b1010_0000, 4),
            prefix(0b1010_0000, 5),
        ];
        v.sort();
        assert_eq!(
            v,
            vec![
                prefix(0b1010_0000, 4),
                prefix(0b1010_0000, 5),
                prefix(0b1010_1000, 5)
            ]
        );
    }

    #[test]
    fn arithmetic_stays_in_the_family() {
        assert_eq!(addr(250).checked_add(5), Some(addr(255)));
        assert_eq!(addr(250).checked_add(6), None);
        assert_eq!(addr(3).checked_sub(4), None);
        assert_eq!(addr(10).offset_from(addr(4)), Ok(Some(6)));
        assert_eq!(addr(4).offset_from(addr(10)), Ok(None));
    }

    proptest! {
        #[test]
        fn containing_is_aligned_and_contains_the_address(bits in 0u128..256, len in 0u8..=8) {
            let a = addr(bits);
            let p = Prefix::containing(a, len).unwrap();
            prop_assert!(p.contains_addr(a));
            prop_assert_eq!(Prefix::new(p.network(), len), Ok(p));
            prop_assert!(p.network() <= a && a <= p.last());
        }

        #[test]
        fn supernets_contain_their_subnets(bits in 0u128..256, len in 0u8..=8, up in 0u8..=8) {
            let p = Prefix::containing(addr(bits), len).unwrap();
            let shorter = up.min(len);
            let sup = p.supernet(shorter).unwrap();
            prop_assert!(sup.contains(p));
            prop_assert_eq!(sup.strictly_contains(p), shorter < len);
        }

        #[test]
        fn overlap_is_nesting_for_cidr_blocks(a in 0u128..256, la in 0u8..=8, b in 0u128..256, lb in 0u8..=8) {
            let p = Prefix::containing(addr(a), la).unwrap();
            let q = Prefix::containing(addr(b), lb).unwrap();
            let share = (0u128..256).any(|x| p.contains_addr(addr(x)) && q.contains_addr(addr(x)));
            prop_assert_eq!(p.overlaps(q), share);
        }
    }
}
