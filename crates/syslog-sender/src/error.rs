// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Errors of message construction and formatting.

use std::fmt;

/// Why a message cannot be built or formatted.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A header field is empty, is a lone `-` (the NILVALUE), or holds a character
    /// outside printable US-ASCII (RFC 5424 §6: `PRINTUSASCII`, 33 to 126).
    InvalidField {
        /// Field name.
        field: &'static str,
        /// Value given.
        value: String,
    },
    /// A header field is longer than RFC 5424 §6 allows.
    FieldTooLong {
        /// Field name.
        field: &'static str,
        /// Maximum length in octets.
        max: usize,
    },
    /// A structured-data name (SD-ID or PARAM-NAME) breaks RFC 5424 §6.3.
    InvalidSdName(String),
    /// The same SD-ID twice in a message (RFC 5424 §6.3.2).
    DuplicateSdId(String),
    /// A parameter value outside the range its registered SD-ID allows (RFC 5424 §7).
    InvalidSdValue {
        /// Parameter name.
        param: &'static str,
        /// Why.
        reason: &'static str,
    },
    /// The header and structured data alone exceed the maximum length: truncating them
    /// would produce an invalid message.
    HeaderTooLong {
        /// Length of header and structured data, in octets.
        len: usize,
        /// Maximum length requested.
        max: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidField { field, value } => write!(
                f,
                "{field} {value:?} must be 1 or more printable US-ASCII characters"
            ),
            Self::FieldTooLong { field, max } => write!(f, "{field} is longer than {max} octets"),
            Self::InvalidSdName(name) => write!(
                f,
                "{name:?} is not a structured-data name: 1 to 32 printable US-ASCII characters except '=', ' ', ']' and '\"'"
            ),
            Self::DuplicateSdId(id) => write!(f, "SD-ID {id:?} appears twice in the message"),
            Self::InvalidSdValue { param, reason } => write!(f, "{param}: {reason}"),
            Self::HeaderTooLong { len, max } => write!(
                f,
                "header and structured data take {len} octets, more than the maximum of {max}"
            ),
        }
    }
}

impl std::error::Error for Error {}
