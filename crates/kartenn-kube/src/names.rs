//! Names and labels of Kubernetes objects (réf. `docs/reference/kubernetes-calcul.md`,
//! § Namespace, labels, noms et UID; pages « Object Names and IDs » and « Labels and
//! Selectors » of the Kubernetes documentation).
//!
//! What Kubernetes documents, and kartenn checks: a DNS subdomain name has at most 253
//! characters, only lowercase alphanumerics, `-` and `.`, and starts and ends with an
//! alphanumeric; a DNS label name has at most 63 characters, only lowercase alphanumerics
//! and `-`, and ends with an alphanumeric; a label key is an optional DNS subdomain prefix
//! and `/`, then a name of 1 to 63 characters, alphanumeric (either case) at both ends with
//! `-`, `_`, `.` between; a label value has the same shape, at most 63 characters, and may
//! be empty.
//!
//! What kartenn chooses (the mirror never refuses what a cluster admitted, so it adds
//! nothing to the documented syntax):
//! - a label name may start with a digit, as RFC 1123 §2.1 allows; the Kubernetes
//!   documentation asks for a letter, and RFC 1035 for a Service;
//! - a subdomain is read as dot-separated labels, each non-empty and alphanumeric at both
//!   ends (the documented definition of a label key prefix), with no bound on the length
//!   of one label beyond the 253 of the whole;
//! - names are not lowercased: an uppercase name does not come from a cluster and is
//!   refused; label keys and values keep their case and compare as written;
//! - the prefixes `kubernetes.io/` and `k8s.io/`, reserved to Kubernetes, are admitted, as
//!   kartenn only reads labels.

use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::fmt;

use kartenn_core::rules::Violation;

use crate::rules::{LABEL_INVALID, NAME_INVALID};

/// Longest DNS label name, and longest label name segment or value.
const MAX_LABEL: usize = 63;
/// Longest DNS subdomain name, and longest label key prefix.
const MAX_SUBDOMAIN: usize = 253;

/// Labels of an object, by key, as kartenn keeps them (only the tracked ones, 019/X4).
pub type Labels = BTreeMap<LabelKey, LabelValue>;

fn lower_alnum(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit()
}

/// Checks one DNS label: non-empty, lowercase alphanumerics and `-`, alphanumeric at both
/// ends. The length bound is the caller's.
fn dns_label_shape(label: &str) -> Result<(), &'static str> {
    let bytes = label.as_bytes();
    let (Some(&first), Some(&last)) = (bytes.first(), bytes.last()) else {
        return Err("étiquette vide");
    };
    if !bytes.iter().all(|&b| lower_alnum(b) || b == b'-') {
        return Err("seuls minuscules ASCII, chiffres et « - » sont permis");
    }
    if !lower_alnum(first) || !lower_alnum(last) {
        return Err("une étiquette commence et finit par une lettre ou un chiffre");
    }
    Ok(())
}

fn dns_subdomain_shape(name: &str) -> Result<(), &'static str> {
    if name.is_empty() {
        return Err("vide");
    }
    if name.len() > MAX_SUBDOMAIN {
        return Err("plus de 253 caractères");
    }
    name.split('.').try_for_each(dns_label_shape)
}

/// Checks a label name segment or a non-empty label value: at most 63 characters,
/// alphanumeric (either case) at both ends, `-`, `_`, `.` and alphanumerics between.
fn label_segment_shape(segment: &str) -> Result<(), &'static str> {
    if segment.len() > MAX_LABEL {
        return Err("plus de 63 caractères");
    }
    let bytes = segment.as_bytes();
    let (Some(first), Some(last)) = (bytes.first(), bytes.last()) else {
        return Err("vide");
    };
    if !bytes
        .iter()
        .all(|&b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err("seuls lettres ASCII, chiffres, « - », « _ » et « . » sont permis");
    }
    if !first.is_ascii_alphanumeric() || !last.is_ascii_alphanumeric() {
        return Err("commence et finit par une lettre ou un chiffre");
    }
    Ok(())
}

macro_rules! string_newtype {
    ($name:ident) => {
        impl $name {
            /// The value, as written.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

/// A DNS label name (« RFC 1123 Label Names »): the name of a namespace or a service.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DnsLabel(String);

impl DnsLabel {
    /// Parses a DNS label name: 1 to 63 lowercase alphanumerics or `-`, alphanumeric at
    /// both ends.
    ///
    /// # Errors
    /// `kube.name.invalid`, with the reason.
    pub fn parse(input: &str) -> Result<Self, Violation> {
        let why = if input.len() > MAX_LABEL {
            Err("plus de 63 caractères")
        } else {
            dns_label_shape(input)
        };
        why.map(|()| Self(input.to_owned()))
            .map_err(|why| Violation::new(&NAME_INVALID, format!("nom « {input} » : {why}")))
    }
}

string_newtype!(DnsLabel);

/// A DNS subdomain name (« DNS Subdomain Names »): the name of a node, a workload, an
/// ingress or a service account.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DnsSubdomain(String);

impl DnsSubdomain {
    /// Parses a DNS subdomain name: at most 253 characters, dot-separated DNS labels.
    ///
    /// # Errors
    /// `kube.name.invalid`, with the reason.
    pub fn parse(input: &str) -> Result<Self, Violation> {
        dns_subdomain_shape(input)
            .map(|()| Self(input.to_owned()))
            .map_err(|why| Violation::new(&NAME_INVALID, format!("nom « {input} » : {why}")))
    }
}

string_newtype!(DnsSubdomain);

/// A label key: an optional DNS subdomain prefix and `/`, then a name segment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LabelKey(String);

impl LabelKey {
    /// Parses a label key, `name` or `prefix/name`.
    ///
    /// # Errors
    /// `kube.label.invalid`, with the reason.
    pub fn parse(input: &str) -> Result<Self, Violation> {
        let checked = match input.split_once('/') {
            None => label_segment_shape(input),
            Some((prefix, name)) => dns_subdomain_shape(prefix)
                .map_err(|_| "le préfixe n'est pas un sous-domaine DNS")
                .and_then(|()| {
                    if name.contains('/') {
                        Err("une seule « / » sépare le préfixe du nom")
                    } else {
                        label_segment_shape(name)
                    }
                }),
        };
        checked.map(|()| Self(input.to_owned())).map_err(|why| {
            Violation::new(&LABEL_INVALID, format!("clé de label « {input} » : {why}"))
        })
    }

    /// The prefix, if any, without the slash.
    #[must_use]
    pub fn prefix(&self) -> Option<&str> {
        self.0.split_once('/').map(|(prefix, _)| prefix)
    }

    /// The name segment.
    #[must_use]
    pub fn name(&self) -> &str {
        self.0.split_once('/').map_or(&self.0, |(_, name)| name)
    }
}

string_newtype!(LabelKey);

/// Lets a set or a map of keys be searched by a `&str` (the order is the string's).
impl Borrow<str> for LabelKey {
    fn borrow(&self) -> &str {
        &self.0
    }
}

/// A label value: empty, or a name segment's shape.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct LabelValue(String);

impl LabelValue {
    /// Parses a label value.
    ///
    /// # Errors
    /// `kube.label.invalid`, with the reason.
    pub fn parse(input: &str) -> Result<Self, Violation> {
        if input.is_empty() {
            return Ok(Self::default());
        }
        label_segment_shape(input)
            .map(|()| Self(input.to_owned()))
            .map_err(|why| {
                Violation::new(
                    &LABEL_INVALID,
                    format!("valeur de label « {input} » : {why}"),
                )
            })
    }
}

string_newtype!(LabelValue);

/// Lets a set of values be searched by a `&str` (the order is the string's).
impl Borrow<str> for LabelValue {
    fn borrow(&self) -> &str {
        &self.0
    }
}

/// Parses the labels of an object, as read.
///
/// # Errors
/// `kube.label.invalid` on the first key or value out of syntax.
pub fn parse_labels<'a>(
    labels: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<Labels, Violation> {
    labels
        .into_iter()
        .map(|(k, v)| Ok((LabelKey::parse(k)?, LabelValue::parse(v)?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name_refused(v: Result<impl fmt::Debug, Violation>) -> bool {
        v.is_err_and(|v| v.rule.id == "kube.name.invalid")
    }

    fn label_refused(v: Result<impl fmt::Debug, Violation>) -> bool {
        v.is_err_and(|v| v.rule.id == "kube.label.invalid")
    }

    #[test]
    fn dns_label_syntax() {
        for ok in ["payments", "a", "web-01", "1abc", &"a".repeat(63)] {
            assert!(DnsLabel::parse(ok).is_ok(), "{ok}");
        }
        for ko in [
            "",
            "-a",
            "a-",
            "Payments",
            "a.b",
            "a_b",
            "é",
            &"a".repeat(64),
        ] {
            assert!(name_refused(DnsLabel::parse(ko)), "{ko}");
        }
    }

    #[test]
    fn dns_subdomain_syntax() {
        let long = ["a".repeat(100), "b".repeat(100), "c".repeat(51)].join(".");
        assert_eq!(long.len(), 253);
        for ok in [
            "node-1",
            "node-1.zone-a.example",
            "web-5d8f7",
            &long,
            &"a".repeat(64),
        ] {
            assert!(DnsSubdomain::parse(ok).is_ok(), "{ok}");
        }
        for ko in [
            "",
            ".a",
            "a.",
            "a..b",
            "a.-b",
            "a-.b",
            "Node-1",
            "node_1",
            &format!("{long}d"),
        ] {
            assert!(name_refused(DnsSubdomain::parse(ko)), "{ko}");
        }
    }

    #[test]
    fn label_key_syntax() {
        for ok in [
            "app",
            "App_Name.v2",
            "tier",
            "app.kubernetes.io/name",
            "kubernetes.io/metadata.name",
            "example.org/Team",
            &"a".repeat(63),
            &format!("{}/x", "p".repeat(253)),
        ] {
            assert!(LabelKey::parse(ok).is_ok(), "{ok}");
        }
        for ko in [
            "",
            "/app",
            "app/",
            "a/b/c",
            "Example.org/app",
            "ex..org/app",
            "-app",
            "app-",
            "_app",
            "app name",
            "app=1",
            &"a".repeat(64),
            &format!("{}/x", "p".repeat(254)),
        ] {
            assert!(label_refused(LabelKey::parse(ko)), "{ko}");
        }
        let key = LabelKey::parse("app.kubernetes.io/name").unwrap();
        assert_eq!(key.prefix(), Some("app.kubernetes.io"));
        assert_eq!(key.name(), "name");
        let bare = LabelKey::parse("tier").unwrap();
        assert_eq!((bare.prefix(), bare.name()), (None, "tier"));
    }

    #[test]
    fn label_value_syntax() {
        for ok in ["", "db", "v1.2.3", "A_b-c", &"z".repeat(63)] {
            assert!(LabelValue::parse(ok).is_ok(), "{ok}");
        }
        for ko in ["-db", "db.", "a b", "a/b", "é", &"z".repeat(64)] {
            assert!(label_refused(LabelValue::parse(ko)), "{ko}");
        }
    }

    #[test]
    fn labels_parsed_or_refused_whole() {
        let labels = parse_labels([("tier", "db"), ("app.kubernetes.io/name", "pay")]).unwrap();
        assert_eq!(labels.len(), 2);
        assert_eq!(labels.get("tier").map(LabelValue::as_str), Some("db"));
        assert!(label_refused(parse_labels([
            ("tier", "db"),
            ("bad key", "x")
        ])));
    }
}
