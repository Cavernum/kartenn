//! Name of a machine: a host name (RFC 1123 §2.1, 009/M3), compared without case and kept
//! in lower case.

use std::fmt;

use kartenn_core::rules::Violation;

use crate::rules::NAME_INVALID;

/// Longest label (RFC 1035 §2.3.4, kept by RFC 1123 §2.1).
const MAX_LABEL: usize = 63;
/// Longest name, dots included (RFC 1123 §2.1).
const MAX_NAME: usize = 253;

/// A valid host name, in lower case.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostName(String);

impl HostName {
    /// Parses a host name: LDH labels of 1 to 63 characters separated by dots, no hyphen
    /// at either end of a label, 253 characters in all. A label may start with a digit
    /// (RFC 1123 §2.1), but a name made only of numeric labels reads as an address and is
    /// refused, as RFC 1123 §2.1 asks to tell them apart.
    ///
    /// # Errors
    /// `machines.name.invalid`, with the reason.
    pub fn parse(input: &str) -> Result<Self, Violation> {
        let invalid = |why: &str| Violation::new(&NAME_INVALID, format!("nom « {input} » : {why}"));
        if input.is_empty() {
            return Err(invalid("vide"));
        }
        if input.len() > MAX_NAME {
            return Err(invalid("plus de 253 caractères"));
        }
        let mut numeric = true;
        for label in input.split('.') {
            if label.is_empty() {
                return Err(invalid("étiquette vide"));
            }
            if label.len() > MAX_LABEL {
                return Err(invalid("étiquette de plus de 63 caractères"));
            }
            if !label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            {
                return Err(invalid(
                    "seuls lettres ASCII, chiffres et « - » sont permis",
                ));
            }
            if label.starts_with('-') || label.ends_with('-') {
                return Err(invalid("une étiquette ne commence ni ne finit par « - »"));
            }
            numeric &= label.bytes().all(|b| b.is_ascii_digit());
        }
        if numeric {
            return Err(invalid(
                "un nom entièrement numérique se lit comme une adresse",
            ));
        }
        Ok(Self(input.to_ascii_lowercase()))
    }

    /// The name, in lower case.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HostName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refused(input: &str) -> bool {
        HostName::parse(input).is_err_and(|v| v.rule.id == "machines.name.invalid")
    }

    #[test]
    fn hostname_labels() {
        for ok in ["srv01", "1srv", "web-01.dc1", "a", &"a".repeat(63), "x.0"] {
            assert!(HostName::parse(ok).is_ok(), "{ok}");
        }
        let long_name = [
            "a".repeat(63),
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(61),
        ]
        .join(".");
        assert_eq!(long_name.len(), 253);
        assert!(HostName::parse(&long_name).is_ok());
        let too_long = format!("{long_name}e");
        for ko in [
            "",
            "-srv",
            "srv-",
            "srv_01",
            "srv 01",
            "srv..dc",
            "srv.",
            ".srv",
            "srvé",
            "10.1.2.3",
            "42",
            &"a".repeat(64),
            &too_long,
        ] {
            assert!(refused(ko), "{ko}");
        }
    }

    #[test]
    fn hostname_lowercased() {
        assert_eq!(HostName::parse("SRV01").unwrap().as_str(), "srv01");
        assert_eq!(
            HostName::parse("Web-01.DC1").unwrap().to_string(),
            "web-01.dc1"
        );
        assert_eq!(
            HostName::parse("SRV01").unwrap(),
            HostName::parse("srv01").unwrap()
        );
    }
}
