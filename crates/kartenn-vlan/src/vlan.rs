//! A VLAN: identifier, name and status (REQ-008-14, decisions V5 and V7).

use std::fmt;

use kartenn_core::rules::Violation;
use unicode_normalization::UnicodeNormalization;

use crate::rules::{NAME_INVALID, VID_INVALID};

/// A VLAN identifier usable in a tag: 1 to 4094 (IEEE 802.1Q-2022 §9.6, table 9-2;
/// 0 is the null VID, 4095 is reserved).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Vid(u16);

impl Vid {
    /// Lowest usable VID.
    pub const MIN: Vid = Vid(1);
    /// Highest usable VID.
    pub const MAX: Vid = Vid(4094);

    /// Checks a VID read from a request or a row (`vlan.vid.invalid`).
    ///
    /// # Errors
    /// A violation of `vlan.vid.invalid` outside 1–4094.
    pub fn new(value: i64) -> Result<Self, Violation> {
        u16::try_from(value)
            .ok()
            .filter(|v| (Self::MIN.0..=Self::MAX.0).contains(v))
            .map(Vid)
            .ok_or_else(|| {
                Violation::new(
                    &VID_INVALID,
                    format!("VID {value} hors de 1–4094 (IEEE 802.1Q §9.6)"),
                )
            })
    }

    /// The numeric value.
    #[must_use]
    pub fn get(self) -> u16 {
        self.0
    }

    /// The next VID, if any.
    #[must_use]
    pub fn succ(self) -> Option<Vid> {
        (self < Self::MAX).then(|| Vid(self.0 + 1))
    }

    /// The previous VID, if any.
    #[must_use]
    pub fn pred(self) -> Option<Vid> {
        (self > Self::MIN).then(|| Vid(self.0 - 1))
    }
}

impl fmt::Display for Vid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Longest VLAN name, in characters (V7).
pub const NAME_MAX_CHARS: usize = 64;

/// A VLAN name: NFC-normalised text of 1 to 64 characters without control characters
/// (V7). Uniqueness within a group is checked against storage, not here.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VlanName(String);

impl VlanName {
    /// Normalises (NFC) and checks a name (`vlan.name.invalid`).
    ///
    /// # Errors
    /// A violation of `vlan.name.invalid` when the name is empty, longer than 64
    /// characters once normalised, or holds a control character.
    pub fn new(raw: &str) -> Result<Self, Violation> {
        let name: String = raw.nfc().collect();
        let len = name.chars().count();
        if len == 0 {
            return Err(Violation::new(&NAME_INVALID, "nom de VLAN vide"));
        }
        if len > NAME_MAX_CHARS {
            return Err(Violation::new(
                &NAME_INVALID,
                format!("nom de VLAN de {len} caractères, au plus {NAME_MAX_CHARS}"),
            ));
        }
        if let Some(c) = name.chars().find(|c| c.is_control()) {
            return Err(Violation::new(
                &NAME_INVALID,
                format!(
                    "nom de VLAN avec le caractère de contrôle U+{:04X}",
                    u32::from(c)
                ),
            ));
        }
        Ok(VlanName(name))
    }

    /// The normalised text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VlanName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Status of a VLAN (V5): `reserved` holds the VID without use, `deprecated` keeps it
/// visible before closing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum VlanStatus {
    /// In use (default).
    #[default]
    Active,
    /// VID held, not in use.
    Reserved,
    /// Still present, to be closed.
    Deprecated,
}

impl VlanStatus {
    /// Wire and storage spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            VlanStatus::Active => "active",
            VlanStatus::Reserved => "reserved",
            VlanStatus::Deprecated => "deprecated",
        }
    }

    /// Parses the wire and storage spelling.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "active" => Some(VlanStatus::Active),
            "reserved" => Some(VlanStatus::Reserved),
            "deprecated" => Some(VlanStatus::Deprecated),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vid_bounds() {
        for bad in [0, 4095, -1, 65_536 + 10] {
            assert_eq!(Vid::new(bad).unwrap_err().rule.id, "vlan.vid.invalid");
        }
        assert_eq!(Vid::new(1).unwrap(), Vid::MIN);
        assert_eq!(Vid::new(4094).unwrap(), Vid::MAX);
        assert_eq!(Vid::MAX.succ(), None);
        assert_eq!(Vid::MIN.succ().map(Vid::get), Some(2));
    }

    #[test]
    fn name_rules() {
        let invalid = |raw: &str| VlanName::new(raw).unwrap_err().rule.id;
        assert_eq!(invalid(""), "vlan.name.invalid");
        assert_eq!(invalid(&"a".repeat(65)), "vlan.name.invalid");
        assert_eq!(invalid("prod\tweb"), "vlan.name.invalid");
        assert_eq!(
            VlanName::new(&"é".repeat(64))
                .unwrap()
                .as_str()
                .chars()
                .count(),
            64
        );
        // A decomposed « é » is normalised to its composed form (NFC) before counting.
        let decomposed = "e\u{301}".repeat(64);
        assert_eq!(VlanName::new(&decomposed).unwrap().as_str(), "é".repeat(64));
        assert_eq!(
            VlanName::new("Invités 2e étage").unwrap().as_str(),
            "Invités 2e étage"
        );
    }

    #[test]
    fn status_round_trip() {
        for s in [
            VlanStatus::Active,
            VlanStatus::Reserved,
            VlanStatus::Deprecated,
        ] {
            assert_eq!(VlanStatus::parse(s.as_str()), Some(s));
        }
        assert_eq!(VlanStatus::parse("retired"), None);
        assert_eq!(VlanStatus::default(), VlanStatus::Active);
    }
}
