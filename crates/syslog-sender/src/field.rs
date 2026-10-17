//! Header fields (RFC 5424 §6.2.4 to §6.2.7): 1 to N printable US-ASCII characters, or
//! absent (written as the NILVALUE `-`).

use crate::Error;

/// Longest HOSTNAME (§6.2.4).
pub(crate) const HOSTNAME_MAX: usize = 255;
/// Longest APP-NAME (§6.2.5).
pub(crate) const APP_NAME_MAX: usize = 48;
/// Longest PROCID (§6.2.6).
pub(crate) const PROCID_MAX: usize = 128;
/// Longest MSGID (§6.2.7).
pub(crate) const MSGID_MAX: usize = 32;

/// Printable US-ASCII without space (`PRINTUSASCII`, §6).
pub(crate) const fn printable(b: u8) -> bool {
    matches!(b, 33..=126)
}

/// Checks a header field and returns it owned. A lone `-` is refused: a receiver would
/// read it as the NILVALUE, an absent field.
pub(crate) fn header(field: &'static str, value: &str, max: usize) -> Result<String, Error> {
    if value.is_empty() || value == "-" || !value.bytes().all(printable) {
        return Err(Error::InvalidField {
            field,
            value: value.to_owned(),
        });
    }
    if value.len() > max {
        return Err(Error::FieldTooLong { field, max });
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_fields_are_printable_and_bounded() {
        assert_eq!(header("MSGID", "ID47", MSGID_MAX).unwrap(), "ID47");
        for bad in ["", "-", "two words", "tab\there", "é", "-\n"] {
            assert!(
                matches!(
                    header("HOSTNAME", bad, HOSTNAME_MAX),
                    Err(Error::InvalidField { .. })
                ),
                "{bad:?}"
            );
        }
        assert_eq!(
            header("MSGID", &"x".repeat(33), MSGID_MAX),
            Err(Error::FieldTooLong {
                field: "MSGID",
                max: 32
            })
        );
        assert!(header("APP-NAME", &"a".repeat(48), APP_NAME_MAX).is_ok());
        assert!(header("PROCID", &"9".repeat(129), PROCID_MAX).is_err());
    }
}
