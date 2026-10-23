//! Languages and labels (spec 018, R7 to R9, REQ-018-8, 9, 12; ADR 0040): language tags
//! of BCP 47, the default language of the instance and the labels by language that an
//! owning object carries (`labels: {<language>: <text>}`).

use std::collections::BTreeMap;
use std::fmt;
use std::sync::OnceLock;

use serde_json::{Map, Value};
use unicode_normalization::UnicodeNormalization;

use crate::rules::{LABEL_INVALID, Violation};

/// A language tag (RFC 5646 §2.1, `langtag`), in lower case: tags are case insensitive
/// (§2.1.1), so the canonical form compares and hashes as one. Grandfathered and
/// private-use-only tags are refused.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct Locale(String);

/// Longest language tag accepted, in characters: a bound of kartenn, not of RFC 5646.
pub const LOCALE_MAX_CHARS: usize = 35;

fn alpha(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphabetic())
}

fn alphanum(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn digits(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|b| b.is_ascii_digit())
}

/// Whether `subtags` (lower case) follow `langtag` of RFC 5646 §2.1.
fn langtag(subtags: &[&str]) -> bool {
    let mut i = 0;
    let Some(language) = subtags.first() else {
        return false;
    };
    if alpha(language, 2, 3) {
        i += 1;
        // extlang: up to three subtags of three letters.
        let mut ext = 0;
        while ext < 3 && subtags.get(i).is_some_and(|s| alpha(s, 3, 3)) {
            i += 1;
            ext += 1;
        }
    } else if alpha(language, 4, 8) {
        i += 1;
    } else {
        return false;
    }
    if subtags.get(i).is_some_and(|s| alpha(s, 4, 4)) {
        i += 1;
    }
    if subtags
        .get(i)
        .is_some_and(|s| alpha(s, 2, 2) || digits(s, 3))
    {
        i += 1;
    }
    while subtags.get(i).is_some_and(|s| {
        alphanum(s, 5, 8) || (s.len() == 4 && s.as_bytes()[0].is_ascii_digit() && alphanum(s, 4, 4))
    }) {
        i += 1;
    }
    // Extensions: a singleton other than `x`, then subtags of 2 to 8 characters.
    while let Some(singleton) = subtags.get(i) {
        if singleton.len() != 1 || *singleton == "x" || !alphanum(singleton, 1, 1) {
            break;
        }
        i += 1;
        let start = i;
        while subtags.get(i).is_some_and(|s| alphanum(s, 2, 8)) {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    if subtags.get(i) == Some(&"x") {
        i += 1;
        let start = i;
        while subtags.get(i).is_some_and(|s| alphanum(s, 1, 8)) {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    i == subtags.len()
}

impl Locale {
    /// Parses a tag, in any case, into its lower-case form; `None` when it does not
    /// follow RFC 5646 §2.1 or is longer than [`LOCALE_MAX_CHARS`].
    #[must_use]
    pub fn parse(tag: &str) -> Option<Self> {
        if tag.is_empty() || tag.len() > LOCALE_MAX_CHARS || !tag.is_ascii() {
            return None;
        }
        let lower = tag.to_ascii_lowercase();
        let subtags: Vec<&str> = lower.split('-').collect();
        langtag(&subtags).then_some(Self(lower))
    }

    /// The tag, in lower case.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The tag and its truncations, longest first, as the Lookup scheme tries them
    /// (RFC 4647 §3.4): a subtag is removed from the end, and a singleton left last goes
    /// with it (`de-ch-x-a` → `de-ch` → `de`).
    #[must_use]
    pub fn truncations(&self) -> Vec<Self> {
        let mut out = vec![self.clone()];
        let mut subtags: Vec<&str> = self.0.split('-').collect();
        while subtags.len() > 1 {
            subtags.pop();
            if subtags.last().is_some_and(|s| s.len() == 1) {
                subtags.pop();
            }
            if !subtags.is_empty() {
                out.push(Self(subtags.join("-")));
            }
        }
        out
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The default language when the configuration names none (R9).
pub const FALLBACK_LOCALE: &str = "fr";

static DEFAULT_LOCALE: OnceLock<Locale> = OnceLock::new();

/// The default language of the instance (`[i18n] default_locale`, REQ-018-12): every
/// `labels` table needs it, and it is served when no language asked matches.
#[must_use]
pub fn default_locale() -> &'static Locale {
    DEFAULT_LOCALE.get_or_init(|| Locale(FALLBACK_LOCALE.to_owned()))
}

/// Sets the default language of the instance, once, at start, before any read of it.
///
/// # Errors
/// The language already in force, when it was set or read before and differs.
pub fn set_default_locale(locale: Locale) -> Result<(), &'static Locale> {
    let mut given = Some(locale);
    let current = DEFAULT_LOCALE.get_or_init(|| {
        given
            .take()
            .unwrap_or_else(|| Locale(FALLBACK_LOCALE.to_owned()))
    });
    match given {
        Some(other) if other != *current => Err(current),
        _ => Ok(()),
    }
}

/// Longest label, in characters (REQ-018-9).
pub const LABEL_MAX_CHARS: usize = 200;

/// Normalises (NFC) and checks the text of a label (`core.label.invalid`).
///
/// # Errors
/// A violation of `core.label.invalid`.
pub fn label_text(raw: &str) -> Result<String, Violation> {
    let text: String = raw.nfc().collect();
    let len = text.chars().count();
    if len == 0 || len > LABEL_MAX_CHARS || text.chars().any(char::is_control) {
        return Err(Violation::new(
            &LABEL_INVALID,
            format!(
                "libellé de {len} caractères : de 1 à {LABEL_MAX_CHARS}, sans caractère de contrôle"
            ),
        ));
    }
    Ok(text)
}

/// The labels of an owning object by language (R8 a, Q-018-6): empty, or holding the
/// default language of the instance.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Labels(BTreeMap<Locale, String>);

impl Labels {
    /// Checks `{<tag>: <text>}`: tags of BCP 47 (lower-cased), texts of 1 to 200
    /// characters without control characters (NFC), the default language present unless
    /// the table is empty.
    ///
    /// # Errors
    /// A violation of `core.label.invalid`.
    pub fn parse(value: &Value) -> Result<Self, Violation> {
        let object = value.as_object().ok_or_else(|| {
            Violation::new(&LABEL_INVALID, "les libellés sont un objet {langue: texte}")
        })?;
        let mut labels = BTreeMap::new();
        for (tag, text) in object {
            let locale = Locale::parse(tag).ok_or_else(|| {
                Violation::new(
                    &LABEL_INVALID,
                    format!("{tag:?} n'est pas une étiquette de langue"),
                )
            })?;
            let text = text.as_str().ok_or_else(|| {
                Violation::new(
                    &LABEL_INVALID,
                    format!("le libellé en {tag} n'est pas un texte"),
                )
            })?;
            if labels.insert(locale, label_text(text)?).is_some() {
                return Err(Violation::new(
                    &LABEL_INVALID,
                    format!("deux libellés en {}", tag.to_ascii_lowercase()),
                ));
            }
        }
        let labels = Self(labels);
        if !labels.0.is_empty() && labels.get(default_locale()).is_none() {
            return Err(Violation::new(
                &LABEL_INVALID,
                format!(
                    "aucun libellé en {}, la langue par défaut",
                    default_locale()
                ),
            ));
        }
        Ok(labels)
    }

    /// [`Labels::parse`] of a table as an operation receives it.
    ///
    /// # Errors
    /// A violation of `core.label.invalid`.
    pub fn parse_map(map: &BTreeMap<String, String>) -> Result<Self, Violation> {
        Self::parse(&Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                .collect(),
        ))
    }

    /// [`Labels::parse`], refusing an empty table: for the objects that need a label.
    ///
    /// # Errors
    /// A violation of `core.label.invalid`.
    pub fn parse_required(value: &Value) -> Result<Self, Violation> {
        let labels = Self::parse(value)?;
        if labels.is_empty() {
            return Err(Violation::new(
                &LABEL_INVALID,
                format!("un libellé en {} au moins est requis", default_locale()),
            ));
        }
        Ok(labels)
    }

    /// Labels compiled or stored, trusted: entries whose tag does not parse are left out.
    #[must_use]
    pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        Self(
            pairs
                .into_iter()
                .filter_map(|(tag, text)| Locale::parse(tag).map(|l| (l, text.to_owned())))
                .collect(),
        )
    }

    /// Labels as stored (checked when written); a malformed row reads as no label.
    #[must_use]
    pub fn stored(value: &Value) -> Self {
        Self(
            value
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(tag, text)| Some((Locale::parse(tag)?, text.as_str()?.to_owned())))
                .collect(),
        )
    }

    /// The label in `locale`, if any.
    #[must_use]
    pub fn get(&self, locale: &Locale) -> Option<&str> {
        self.0.get(locale).map(String::as_str)
    }

    /// No label at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The labels by language.
    pub fn iter(&self) -> impl Iterator<Item = (&Locale, &str)> {
        self.0.iter().map(|(l, t)| (l, t.as_str()))
    }

    /// The JSON form, `{<tag>: <text>}`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        Value::Object(
            self.0
                .iter()
                .map(|(l, t)| (l.0.clone(), Value::String(t.clone())))
                .collect::<Map<_, _>>(),
        )
    }
}

impl serde::Serialize for Labels {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.0.iter())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn language_tags_follow_rfc_5646() {
        for good in [
            "fr",
            "en",
            "fr-CA",
            "FR",
            "zh-Hant-TW",
            "sr-Latn-RS",
            "es-419",
            "de-CH-1996",
            "en-a-bbb-x-a-ccc",
            "zh-yue",
            "ast",
        ] {
            assert!(Locale::parse(good).is_some(), "{good}");
        }
        for bad in [
            "",
            "f",
            "fr_FR",
            "fr-",
            "-fr",
            "x-private",
            "fr-a",
            "123",
            "fr-toolongsubtag",
            "i-klingon",
            "fr-ca-ca",
            "é",
        ] {
            assert!(Locale::parse(bad).is_none(), "{bad}");
        }
        assert_eq!(Locale::parse("fr-CA").unwrap().as_str(), "fr-ca");
    }

    #[test]
    fn lookup_truncates_from_the_end() {
        let tags = |t: &str| -> Vec<String> {
            Locale::parse(t)
                .unwrap()
                .truncations()
                .into_iter()
                .map(|l| l.0)
                .collect()
        };
        assert_eq!(tags("zh-hant-tw"), ["zh-hant-tw", "zh-hant", "zh"]);
        assert_eq!(tags("de-ch-x-a"), ["de-ch-x-a", "de-ch", "de"]);
        assert_eq!(tags("fr"), ["fr"]);
    }

    /// REQ-018-9: tags of BCP 47, texts of 1 to 200 characters without control
    /// characters, the default language present (`core.label.invalid`).
    #[test]
    fn labels_are_checked() {
        let ok = Labels::parse(&json!({"fr": "Production", "EN": "Production"})).unwrap();
        assert_eq!(ok.get(&Locale::parse("en").unwrap()), Some("Production"));
        assert!(Labels::parse(&json!({})).unwrap().is_empty());
        for bad in [
            json!({"en": "Production"}),
            json!({"fr": ""}),
            json!({"fr": "a\u{7}b"}),
            json!({"fr": "x".repeat(201)}),
            json!({"fr_FR": "x", "fr": "x"}),
            json!({"fr": 1}),
            json!({"fr": "x", "FR": "y"}),
            json!(["fr"]),
        ] {
            let error = Labels::parse(&bad).unwrap_err();
            assert_eq!(error.rule.id, "core.label.invalid", "{bad}");
        }
        assert!(Labels::parse_required(&json!({})).is_err());
        assert_eq!(
            Labels::parse(&json!({"fr": "e\u{301}t\u{e9}"}))
                .unwrap()
                .to_json(),
            json!({"fr": "\u{e9}t\u{e9}"})
        );
    }
}
