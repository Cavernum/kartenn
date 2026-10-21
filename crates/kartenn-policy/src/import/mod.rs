//! The offline converter of an existing policy into a changeset (task 010-17; REQ-010-35,
//! 53; S28): a matrix of three states, a catalogue in two tables and rules of inference
//! become the operations of one changeset, and a simulation lists where the converted
//! policy answers otherwise than the tool of origin. No historical format becomes a
//! contract of the server (S28): this converter is a tool of the operator, outside it.

pub mod convert;
pub mod simulate;
pub mod source;

use std::fmt;

/// An input the converter cannot read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportError(pub String);

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ImportError {}

impl ImportError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}
