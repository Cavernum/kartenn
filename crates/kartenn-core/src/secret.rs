//! Values that must never appear in a log, a trace or an error (spec 012, REQ-012-28).

use std::fmt;

/// A secret (password, token, key, connection string): its `Debug` and `Display` print
/// `***`, and it implements no serialisation, so it cannot reach an output by accident.
/// [`Secret::expose`] gives the value to the code that needs it.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret<T>(T);

impl<T> Secret<T> {
    /// Wraps a secret.
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// The secret itself, for the code that uses it.
    pub const fn expose(&self) -> &T {
        &self.0
    }
}

impl<T> From<T> for Secret<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl<T> fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_never_prints() {
        #[derive(Debug)]
        #[allow(dead_code)] // read through Debug only
        struct Login {
            user: &'static str,
            password: Secret<&'static str>,
        }
        let s = Secret::new(String::from("hunter2"));
        assert_eq!(format!("{s:?} {s} {s:#?}"), "*** *** ***");
        assert_eq!(s.expose(), "hunter2");
        let login = Login {
            user: "alice",
            password: "hunter2".into(),
        };
        assert!(!format!("{login:?}").contains("hunter2"));
    }
}
