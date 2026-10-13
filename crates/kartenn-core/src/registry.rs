//! IANA special-purpose address registries (RFC 6890, RFC 8190; REQ-002-33, 34).
//!
//! The registries are embedded as the CSV files published by IANA
//! (`data/iana/`), parsed once, and queried for the most specific entry containing a
//! value. Entries whose termination date has passed (deprecated allocations) are kept
//! for reference but ignored by the classification.

use std::sync::LazyLock;

use crate::addr::{AddrError, Family, Prefix};
use crate::family::{IPV4, IPV6};

/// Value of a boolean column of the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    /// `True`.
    True,
    /// `False`.
    False,
    /// `N/A` (for example 6to4 and Teredo, Globally Reachable).
    NotApplicable,
    /// Empty cell (terminated allocations).
    Unspecified,
}

/// One registry entry; a CSV row with several blocks yields one entry per block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialEntry {
    /// Address block.
    pub block: Prefix,
    /// Name given by IANA.
    pub name: String,
    /// Defining RFCs, as written by IANA.
    pub rfc: String,
    /// Allocation date (`YYYY-MM`).
    pub allocated: String,
    /// Termination date (`YYYY-MM`), if the allocation is terminated.
    pub terminated: Option<String>,
    /// Valid as a source address.
    pub source: Flag,
    /// Valid as a destination address.
    pub destination: Flag,
    /// A router may forward datagrams to it.
    pub forwardable: Flag,
    /// Forwardable beyond an administrative domain (RFC 8190 §2.1).
    pub globally_reachable: Flag,
    /// Reserved by the IP protocol itself.
    pub reserved_by_protocol: Flag,
}

/// The special-purpose registry of one address family.
#[derive(Debug)]
pub struct SpecialRegistry {
    family: Family,
    updated: &'static str,
    entries: Vec<SpecialEntry>,
}

/// Errors in an embedded registry file: a defect of the release, caught by the tests.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RegistryError {
    /// A row has the wrong number of fields or an unterminated quote.
    #[error("line {line}: malformed CSV row")]
    Csv {
        /// One-based line number.
        line: usize,
    },
    /// A block is not a valid prefix of the family.
    #[error("line {line}: invalid block: {source}")]
    Block {
        /// One-based line number.
        line: usize,
        /// Parse error.
        source: AddrError,
    },
    /// A flag cell holds an unknown value.
    #[error("line {line}: unknown flag value {value:?}")]
    Flag {
        /// One-based line number.
        line: usize,
        /// Cell content.
        value: String,
    },
}

/// Splits one CSV record (RFC 4180: quoted fields, doubled quotes).
fn split_csv(record: &str) -> Option<Vec<String>> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut chars = record.chars().peekable();
    let mut quoted = false;
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            (true, '"') => quoted = false,
            (false, '"') if field.is_empty() => quoted = true,
            (false, ',') => fields.push(std::mem::take(&mut field)),
            (_, c) => field.push(c),
        }
    }
    if quoted {
        return None;
    }
    fields.push(field);
    Some(fields)
}

/// Removes a trailing footnote marker such as ` [2]`.
fn strip_footnote(cell: &str) -> &str {
    match cell.trim().rsplit_once(" [") {
        Some((value, note))
            if note.ends_with(']')
                && note[..note.len() - 1].bytes().all(|b| b.is_ascii_digit()) =>
        {
            value.trim()
        }
        _ => cell.trim(),
    }
}

fn parse_flag(line: usize, cell: &str) -> Result<Flag, RegistryError> {
    match strip_footnote(cell) {
        "True" => Ok(Flag::True),
        "False" => Ok(Flag::False),
        "N/A" => Ok(Flag::NotApplicable),
        "" => Ok(Flag::Unspecified),
        other => Err(RegistryError::Flag {
            line,
            value: other.to_owned(),
        }),
    }
}

impl SpecialRegistry {
    /// Parses a registry CSV file published by IANA.
    ///
    /// # Errors
    ///
    /// [`RegistryError`] on malformed rows, blocks or flags.
    pub fn parse(family: Family, updated: &'static str, csv: &str) -> Result<Self, RegistryError> {
        let mut entries = Vec::new();
        for (index, record) in csv.lines().enumerate().skip(1) {
            let line = index + 1;
            if record.trim().is_empty() {
                continue;
            }
            let fields = split_csv(record).ok_or(RegistryError::Csv { line })?;
            let [
                blocks,
                name,
                rfc,
                allocated,
                terminated,
                source,
                destination,
                forwardable,
                global,
                reserved,
            ] = fields.as_slice()
            else {
                return Err(RegistryError::Csv { line });
            };
            let terminated = match terminated.trim() {
                "" | "N/A" => None,
                date => Some(date.to_owned()),
            };
            for block in blocks.split(',') {
                let block = Prefix::parse(family, strip_footnote(block))
                    .map_err(|source| RegistryError::Block { line, source })?;
                entries.push(SpecialEntry {
                    block,
                    name: name.trim().to_owned(),
                    rfc: rfc.trim().to_owned(),
                    allocated: allocated.trim().to_owned(),
                    terminated: terminated.clone(),
                    source: parse_flag(line, source)?,
                    destination: parse_flag(line, destination)?,
                    forwardable: parse_flag(line, forwardable)?,
                    globally_reachable: parse_flag(line, global)?,
                    reserved_by_protocol: parse_flag(line, reserved)?,
                });
            }
        }
        Ok(Self {
            family,
            updated,
            entries,
        })
    }

    /// Family of the registry.
    #[must_use]
    pub const fn family(&self) -> Family {
        self.family
    }

    /// IANA « Last Updated » date of the embedded file (REQ-002-34).
    #[must_use]
    pub const fn updated(&self) -> &'static str {
        self.updated
    }

    /// All entries, terminated ones included, in file order.
    #[must_use]
    pub fn entries(&self) -> &[SpecialEntry] {
        &self.entries
    }

    /// The most specific active entry whose block contains `prefix` (an address is a
    /// full-length prefix), if any (REQ-002-33).
    #[must_use]
    pub fn classify(&self, prefix: Prefix) -> Option<&SpecialEntry> {
        self.entries
            .iter()
            .filter(|e| e.terminated.is_none() && e.block.contains(prefix))
            .max_by_key(|e| e.block.prefix_len())
    }

    /// Whether `prefix` lies in public space: no active entry contains it, or the most
    /// specific one is Globally Reachable (002/A3).
    #[must_use]
    pub fn is_public(&self, prefix: Prefix) -> bool {
        self.classify(prefix)
            .is_none_or(|e| e.globally_reachable == Flag::True)
    }
}

/// « Last Updated » date of both embedded registries (see `data/iana/README.md`).
pub const UPDATED: &str = "2025-10-09";

static IPV4_REGISTRY: LazyLock<SpecialRegistry> = LazyLock::new(|| {
    SpecialRegistry::parse(
        IPV4,
        UPDATED,
        include_str!("../data/iana/iana-ipv4-special-registry.csv"),
    )
    .expect("the embedded IPv4 registry is valid (checked by the tests)")
});

static IPV6_REGISTRY: LazyLock<SpecialRegistry> = LazyLock::new(|| {
    SpecialRegistry::parse(
        IPV6,
        UPDATED,
        include_str!("../data/iana/iana-ipv6-special-registry.csv"),
    )
    .expect("the embedded IPv6 registry is valid (checked by the tests)")
});

/// The embedded special-purpose registry of `family`, if the family has one.
#[must_use]
pub fn special_registry(family: Family) -> Option<&'static SpecialRegistry> {
    if family == IPV4 {
        Some(&IPV4_REGISTRY)
    } else if family == IPV6 {
        Some(&IPV6_REGISTRY)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addr::{Addr, Prefix};
    use crate::family::ALL;

    fn host(text: &str) -> Prefix {
        let a = Addr::parse_any(&ALL, text).unwrap();
        Prefix::new(a, a.family().width()).unwrap()
    }

    fn name_of(text: &str) -> Option<String> {
        let p = host(text);
        special_registry(p.family())
            .unwrap()
            .classify(p)
            .map(|e| e.name.clone())
    }

    #[test]
    fn embedded_registries_parse_completely() {
        assert_eq!(
            special_registry(IPV4).unwrap().entries().len(),
            26,
            "25 rows, one with two blocks"
        );
        assert_eq!(special_registry(IPV6).unwrap().entries().len(), 25);
        assert_eq!(special_registry(IPV4).unwrap().updated(), "2025-10-09");
    }

    #[test]
    fn most_specific_entry_wins() {
        // REQ-002-33 acceptance.
        assert_eq!(
            name_of("192.0.0.9").as_deref(),
            Some("Port Control Protocol Anycast")
        );
        assert_eq!(
            name_of("192.0.0.200").as_deref(),
            Some("IETF Protocol Assignments")
        );
        assert_eq!(name_of("10.1.2.3").as_deref(), Some("Private-Use"));
        assert_eq!(name_of("8.8.8.8"), None);
    }

    #[test]
    fn multi_block_cells_and_footnotes_are_handled() {
        assert_eq!(
            name_of("192.0.0.171").as_deref(),
            Some("NAT64/DNS64 Discovery")
        );
        assert_eq!(name_of("2002::1").as_deref(), Some("6to4"));
        let teredo = special_registry(IPV6)
            .unwrap()
            .classify(host("2001::1"))
            .unwrap();
        assert_eq!(teredo.globally_reachable, Flag::NotApplicable);
        let ietf = special_registry(IPV6)
            .unwrap()
            .classify(host("2001:5::1"))
            .unwrap();
        assert_eq!(ietf.source, Flag::False, "footnoted 'False [1]'");
    }

    #[test]
    fn terminated_allocations_are_ignored() {
        assert_eq!(
            name_of("192.88.99.1"),
            None,
            "6to4 relay anycast, terminated 2015-03"
        );
        assert_eq!(
            name_of("192.88.99.2").as_deref(),
            Some("6a44-relay anycast address")
        );
        assert_eq!(
            name_of("2001:10::1").as_deref(),
            Some("IETF Protocol Assignments")
        );
        assert_eq!(name_of("2001:20::1").as_deref(), Some("ORCHIDv2"));
    }

    #[test]
    fn recent_ipv6_entries_are_known() {
        assert_eq!(name_of("3fff::1").as_deref(), Some("Documentation"));
        assert_eq!(name_of("2001:db8::1").as_deref(), Some("Documentation"));
        assert_eq!(
            name_of("5f00::1").as_deref(),
            Some("Segment Routing (SRv6) SIDs")
        );
        assert_eq!(name_of("fd12:3456::1").as_deref(), Some("Unique-Local"));
    }

    #[test]
    fn public_space() {
        // 002/A3: no entry, or Globally Reachable.
        let public = |t: &str| {
            special_registry(host(t).family())
                .unwrap()
                .is_public(host(t))
        };
        assert!(public("8.8.8.8"));
        assert!(public("192.0.0.9"), "PCP anycast is globally reachable");
        assert!(public("2a00::1"));
        assert!(!public("10.1.2.3"));
        assert!(!public("100.64.0.1"));
        assert!(!public("fd00::1"));
        assert!(!public("2001::1"), "Teredo: N/A is not True");
        let reg = special_registry(IPV4).unwrap();
        assert!(reg.is_public(Prefix::parse(IPV4, "20.0.0.0/10").unwrap()));
        assert!(!reg.is_public(Prefix::parse(IPV4, "10.0.0.0/16").unwrap()));
        assert!(
            reg.is_public(Prefix::parse(IPV4, "0.0.0.0/0").unwrap()),
            "no entry contains a /0"
        );
    }

    #[test]
    fn csv_quotes_are_parsed() {
        assert_eq!(
            split_csv(r#"a,"b, c","d ""e""",f"#).unwrap(),
            vec!["a", "b, c", r#"d "e""#, "f"]
        );
        assert_eq!(split_csv(r#"a,"unterminated"#), None);
        assert_eq!(strip_footnote("False [1]"), "False");
        assert_eq!(strip_footnote("2002::/16 [3]"), "2002::/16");
        assert_eq!(strip_footnote("[RFC 4380]"), "[RFC 4380]");
    }
}
