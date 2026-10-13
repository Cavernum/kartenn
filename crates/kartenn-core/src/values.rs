//! Typed property values (spec 003, REQ-003-7 to 9).
//!
//! A value is parsed from its raw input by a [`ValueType`], normalised and validated
//! strictly (D1). Scalars map onto a fixed set of storage kinds, so that a module can
//! add a type without changing the storage. Sets are sorted and deduplicated.

use std::collections::BTreeSet;
use std::fmt;

use regex::Regex;
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

/// A calendar date, RFC 3339 `full-date` (REQ-003-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    year: u16,
    month: u8,
    day: u8,
}

impl Date {
    /// Parses `YYYY-MM-DD` strictly (RFC 3339 §5.6–5.7).
    ///
    /// # Errors
    ///
    /// A message describing the expected form.
    pub fn parse(text: &str) -> Result<Self, String> {
        let bad = || format!("{text:?} is not a date YYYY-MM-DD (RFC 3339 full-date)");
        let b = text.as_bytes();
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
            return Err(bad());
        }
        let digits = |s: &str| {
            s.bytes()
                .all(|c| c.is_ascii_digit())
                .then(|| s.parse::<u16>().ok())
                .flatten()
        };
        let (year, month, day) = (
            digits(&text[0..4]),
            digits(&text[5..7]),
            digits(&text[8..10]),
        );
        let (Some(year), Some(month), Some(day)) = (year, month, day) else {
            return Err(bad());
        };
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let max_day = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => return Err(bad()),
        };
        if day == 0 || day > max_day {
            return Err(bad());
        }
        Ok(Self {
            year,
            month: u8::try_from(month).map_err(|_| bad())?,
            day: u8::try_from(day).map_err(|_| bad())?,
        })
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// A validated scalar, in one of the storage kinds (003/design, § Types de valeurs).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scalar {
    /// Identifier of a closed-list value.
    Enum(Uuid),
    /// Normalised text (also FQDN).
    Text(String),
    /// 64-bit signed integer.
    Int(i64),
    /// Boolean.
    Bool(bool),
    /// Calendar date.
    Date(Date),
    /// Reference to an object.
    Ref(Uuid),
}

/// A property value: one scalar or a set of scalars (REQ-003-9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// Single value.
    One(Scalar),
    /// Set: sorted, without duplicates, compared as a set.
    Set(BTreeSet<Scalar>),
}

/// Raw input of a scalar, as received from an interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Raw {
    /// Text (closed-list key, text, FQDN, date, reference identifier).
    Text(String),
    /// Integer.
    Int(i64),
    /// Boolean.
    Bool(bool),
}

/// Cardinality of a property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    /// One value.
    Single,
    /// A set of values.
    Set,
}

/// Facts a type needs from the rest of the system to validate a value.
pub trait ValueContext {
    /// Identifier of the closed-list value `key` of the property, valid at the date of
    /// the assignment, if any.
    fn enum_value(&self, key: &str) -> Option<Uuid>;
    /// Kind of the object `id` if it exists over the whole period of the assignment.
    fn object_kind(&self, id: Uuid) -> Option<&'static str>;
}

/// A value type: parses, normalises and validates a raw scalar (REQ-003-7, 8).
pub trait ValueType: fmt::Debug + Send + Sync {
    /// Identifier of the type (`enum`, `text`, `int`…).
    fn id(&self) -> &'static str;

    /// Parses and validates one scalar.
    ///
    /// # Errors
    ///
    /// A message citing the violated constraint.
    fn parse(&self, raw: &Raw, ctx: &dyn ValueContext) -> Result<Scalar, String>;
}

/// Closed list (`enum`): the raw text is the key of a value valid at the date.
#[derive(Debug, Clone, Copy)]
pub struct EnumType;

/// Text (`text`): NFC, no control character, length in code points, optional regex.
#[derive(Debug, Clone)]
pub struct TextType {
    /// Minimum length in code points.
    pub min_len: usize,
    /// Maximum length in code points.
    pub max_len: usize,
    /// Pattern the whole text must match (anchored).
    pub pattern: Option<Regex>,
}

/// Integer (`int`), within bounds.
#[derive(Debug, Clone, Copy)]
pub struct IntType {
    /// Smallest accepted value.
    pub min: i64,
    /// Largest accepted value.
    pub max: i64,
}

/// Boolean (`bool`).
#[derive(Debug, Clone, Copy)]
pub struct BoolType;

/// Date (`date`), RFC 3339 `full-date`.
#[derive(Debug, Clone, Copy)]
pub struct DateType;

/// Reference (`ref`) to an object of one kind, valid over the assignment period.
#[derive(Debug, Clone, Copy)]
pub struct RefType {
    /// Kind of the referenced objects (`entity`, `site`…).
    pub target: &'static str,
}

fn expect_text<'a>(raw: &'a Raw, type_id: &str) -> Result<&'a str, String> {
    match raw {
        Raw::Text(t) => Ok(t),
        other => Err(format!(
            "{other:?} is not a text, as a {type_id} value must be"
        )),
    }
}

impl TextType {
    /// A text type; the pattern must match the whole text.
    ///
    /// # Errors
    ///
    /// When the bounds are inverted or the pattern does not compile.
    pub fn new(min_len: usize, max_len: usize, pattern: Option<&str>) -> Result<Self, String> {
        if min_len > max_len {
            return Err(format!(
                "minimum length {min_len} is above maximum length {max_len}"
            ));
        }
        let pattern = pattern
            .map(|p| Regex::new(&format!("^(?:{p})$")).map_err(|e| format!("invalid pattern: {e}")))
            .transpose()?;
        Ok(Self {
            min_len,
            max_len,
            pattern,
        })
    }
}

impl ValueType for EnumType {
    fn id(&self) -> &'static str {
        "enum"
    }

    fn parse(&self, raw: &Raw, ctx: &dyn ValueContext) -> Result<Scalar, String> {
        let key = expect_text(raw, self.id())?;
        ctx.enum_value(key)
            .map(Scalar::Enum)
            .ok_or_else(|| format!("{key:?} is not a value of the list at this date"))
    }
}

impl ValueType for TextType {
    fn id(&self) -> &'static str {
        "text"
    }

    fn parse(&self, raw: &Raw, _ctx: &dyn ValueContext) -> Result<Scalar, String> {
        let text: String = expect_text(raw, self.id())?.nfc().collect();
        if let Some(c) = text.chars().find(|c| c.is_control()) {
            return Err(format!(
                "the text contains the control character U+{:04X}",
                u32::from(c)
            ));
        }
        let len = text.chars().count();
        if len < self.min_len || len > self.max_len {
            return Err(format!(
                "the text has {len} characters, expected {} to {}",
                self.min_len, self.max_len
            ));
        }
        if let Some(p) = &self.pattern
            && !p.is_match(&text)
        {
            return Err(format!(
                "the text does not match the pattern {}",
                p.as_str()
            ));
        }
        Ok(Scalar::Text(text))
    }
}

impl ValueType for IntType {
    fn id(&self) -> &'static str {
        "int"
    }

    fn parse(&self, raw: &Raw, _ctx: &dyn ValueContext) -> Result<Scalar, String> {
        let Raw::Int(v) = raw else {
            return Err(format!("{raw:?} is not an integer"));
        };
        if *v < self.min || *v > self.max {
            return Err(format!("{v} is outside {}..={}", self.min, self.max));
        }
        Ok(Scalar::Int(*v))
    }
}

impl ValueType for BoolType {
    fn id(&self) -> &'static str {
        "bool"
    }

    fn parse(&self, raw: &Raw, _ctx: &dyn ValueContext) -> Result<Scalar, String> {
        match raw {
            Raw::Bool(b) => Ok(Scalar::Bool(*b)),
            other => Err(format!("{other:?} is not a boolean")),
        }
    }
}

impl ValueType for DateType {
    fn id(&self) -> &'static str {
        "date"
    }

    fn parse(&self, raw: &Raw, _ctx: &dyn ValueContext) -> Result<Scalar, String> {
        Date::parse(expect_text(raw, self.id())?).map(Scalar::Date)
    }
}

impl ValueType for RefType {
    fn id(&self) -> &'static str {
        "ref"
    }

    fn parse(&self, raw: &Raw, ctx: &dyn ValueContext) -> Result<Scalar, String> {
        let text = expect_text(raw, self.id())?;
        let id =
            Uuid::parse_str(text).map_err(|_| format!("{text:?} is not an object identifier"))?;
        match ctx.object_kind(id) {
            Some(kind) if kind == self.target => Ok(Scalar::Ref(id)),
            Some(kind) => Err(format!("{id} is a {kind}, not a {}", self.target)),
            None => Err(format!(
                "{id} does not exist over the whole period of the assignment"
            )),
        }
    }
}

/// Parses a value of the given cardinality (REQ-003-9): a set is sorted and deduplicated.
///
/// # Errors
///
/// The first scalar error, or a cardinality mismatch.
pub fn parse_value(
    ty: &dyn ValueType,
    cardinality: Cardinality,
    raws: &[Raw],
    ctx: &dyn ValueContext,
) -> Result<Value, String> {
    match (cardinality, raws) {
        (Cardinality::Single, [raw]) => ty.parse(raw, ctx).map(Value::One),
        (Cardinality::Single, _) => Err(format!("expected exactly one value, got {}", raws.len())),
        (Cardinality::Set, raws) => raws
            .iter()
            .map(|r| ty.parse(r, ctx))
            .collect::<Result<_, _>>()
            .map(Value::Set),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use proptest::prelude::*;

    /// A context with a fixed list and one entity.
    pub(crate) struct Ctx {
        pub(crate) entity: Uuid,
    }

    pub(crate) fn enum_id(key: &str) -> Uuid {
        Uuid::from_u128(key.bytes().fold(1u128, |acc, b| {
            acc.wrapping_mul(131).wrapping_add(u128::from(b))
        }))
    }

    impl ValueContext for Ctx {
        fn enum_value(&self, key: &str) -> Option<Uuid> {
            ["prod", "dev", "rec", "global"]
                .contains(&key)
                .then(|| enum_id(key))
        }
        fn object_kind(&self, id: Uuid) -> Option<&'static str> {
            (id == self.entity).then_some("entity")
        }
    }

    fn ctx() -> Ctx {
        Ctx {
            entity: Uuid::now_v7(),
        }
    }

    fn text(t: &str) -> Raw {
        Raw::Text(t.to_owned())
    }

    #[test]
    fn dates_follow_rfc3339() {
        assert_eq!(Date::parse("2024-02-29").unwrap().to_string(), "2024-02-29");
        for bad in [
            "2023-02-29",
            "1900-02-29",
            "2024-13-01",
            "2024-04-31",
            "2024-1-01",
            "2024/01/01",
            "24-01-01",
            "2024-01-00",
            "2024-01-01T00:00:00Z",
        ] {
            assert!(Date::parse(bad).is_err(), "{bad}");
        }
        assert!(Date::parse("2000-02-29").is_ok(), "divisible by 400");
    }

    #[test]
    fn text_is_normalised_and_bounded() {
        let t = TextType::new(1, 5, None).unwrap();
        // "é" as e + combining acute: NFC turns it into one code point.
        assert_eq!(
            t.parse(&text("cafe\u{301}"), &ctx()),
            Ok(Scalar::Text("café".into()))
        );
        assert!(
            t.parse(&text("a\u{7}b"), &ctx())
                .unwrap_err()
                .contains("U+0007")
        );
        assert!(t.parse(&text(""), &ctx()).is_err());
        assert!(t.parse(&text("abcdef"), &ctx()).is_err());
    }

    #[test]
    fn text_pattern_is_anchored() {
        let t = TextType::new(0, 20, Some("[A-Z]{2}-[0-9]+")).unwrap();
        assert!(t.parse(&text("FR-42"), &ctx()).is_ok());
        assert!(
            t.parse(&text("xFR-42x"), &ctx()).is_err(),
            "the whole text must match"
        );
        assert!(TextType::new(0, 1, Some("(")).is_err());
        assert!(TextType::new(3, 1, None).is_err());
    }

    #[test]
    fn integers_and_booleans() {
        let i = IntType { min: 0, max: 100 };
        assert_eq!(i.parse(&Raw::Int(42), &ctx()), Ok(Scalar::Int(42)));
        assert!(i.parse(&Raw::Int(101), &ctx()).is_err());
        assert!(i.parse(&text("42"), &ctx()).is_err());
        assert_eq!(
            BoolType.parse(&Raw::Bool(true), &ctx()),
            Ok(Scalar::Bool(true))
        );
    }

    #[test]
    fn closed_lists_and_references() {
        let c = ctx();
        assert_eq!(
            EnumType.parse(&text("prod"), &c),
            Ok(Scalar::Enum(enum_id("prod")))
        );
        assert!(EnumType.parse(&text("sandbox"), &c).is_err());
        let r = RefType { target: "entity" };
        assert_eq!(
            r.parse(&text(&c.entity.to_string()), &c),
            Ok(Scalar::Ref(c.entity))
        );
        assert!(
            r.parse(&text(&Uuid::now_v7().to_string()), &c)
                .unwrap_err()
                .contains("does not exist")
        );
        assert!(
            RefType { target: "site" }
                .parse(&text(&c.entity.to_string()), &c)
                .unwrap_err()
                .contains("not a site")
        );
    }

    #[test]
    fn sets_are_sorted_and_deduplicated() {
        // REQ-003-9 acceptance.
        let c = ctx();
        let a = parse_value(
            &EnumType,
            Cardinality::Set,
            &[text("prod"), text("global")],
            &c,
        )
        .unwrap();
        let b = parse_value(
            &EnumType,
            Cardinality::Set,
            &[text("global"), text("prod"), text("prod")],
            &c,
        )
        .unwrap();
        assert_eq!(a, b);
        let Value::Set(s) = b else { panic!("a set") };
        assert_eq!(s.len(), 2);
        assert!(
            parse_value(
                &EnumType,
                Cardinality::Single,
                &[text("prod"), text("dev")],
                &c
            )
            .is_err()
        );
    }

    proptest! {
        #[test]
        fn text_normalisation_is_idempotent(s in "\\PC{0,30}") {
            let t = TextType::new(0, 1000, None).unwrap();
            if let Ok(Scalar::Text(once)) = t.parse(&Raw::Text(s), &ctx()) {
                prop_assert_eq!(t.parse(&Raw::Text(once.clone()), &ctx()), Ok(Scalar::Text(once)));
            }
        }

        #[test]
        fn valid_dates_round_trip(y in 1u16..=9999, m in 1u8..=12, d in 1u8..=31) {
            let text = format!("{y:04}-{m:02}-{d:02}");
            if let Ok(date) = Date::parse(&text) {
                prop_assert_eq!(date.to_string(), text);
            }
        }
    }
}
