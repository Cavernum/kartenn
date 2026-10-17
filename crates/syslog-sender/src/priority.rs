//! Facilities and severities (RFC 5424 §6.2.1, tables 1 and 2).

/// Facility of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Facility {
    /// Kernel messages.
    Kernel = 0,
    /// User-level messages.
    User = 1,
    /// Mail system.
    Mail = 2,
    /// System daemons.
    Daemon = 3,
    /// Security and authorization messages.
    Auth = 4,
    /// Messages generated internally by syslogd.
    Syslog = 5,
    /// Line printer subsystem.
    Lpr = 6,
    /// Network news subsystem.
    News = 7,
    /// UUCP subsystem.
    Uucp = 8,
    /// Clock daemon.
    Cron = 9,
    /// Security and authorization messages (private).
    AuthPriv = 10,
    /// FTP daemon.
    Ftp = 11,
    /// NTP subsystem.
    Ntp = 12,
    /// Log audit.
    Audit = 13,
    /// Log alert.
    Alert = 14,
    /// Clock daemon (second code).
    Clock = 15,
    /// Local use 0.
    Local0 = 16,
    /// Local use 1.
    Local1 = 17,
    /// Local use 2.
    Local2 = 18,
    /// Local use 3.
    Local3 = 19,
    /// Local use 4.
    Local4 = 20,
    /// Local use 5.
    Local5 = 21,
    /// Local use 6.
    Local6 = 22,
    /// Local use 7.
    Local7 = 23,
}

/// Severity of a message, from the most to the least severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Severity {
    /// System is unusable.
    Emergency = 0,
    /// Action must be taken immediately.
    Alert = 1,
    /// Critical conditions.
    Critical = 2,
    /// Error conditions.
    Error = 3,
    /// Warning conditions.
    Warning = 4,
    /// Normal but significant condition.
    Notice = 5,
    /// Informational messages.
    Informational = 6,
    /// Debug-level messages.
    Debug = 7,
}

impl Facility {
    /// Numerical code.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

impl Severity {
    /// Numerical code.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// The PRI value: facility × 8 + severity (RFC 5424 §6.2.1).
pub(crate) const fn priority(facility: Facility, severity: Severity) -> u8 {
    facility.code() * 8 + severity.code()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priorities_follow_the_rfc_examples() {
        // RFC 5424 §6.2.1: "a mail system critical message" has PRI 18; §6.5 example 1
        // (auth, critical) 34, example 2 (local4, notice) 165.
        assert_eq!(priority(Facility::Mail, Severity::Critical), 18);
        assert_eq!(priority(Facility::Auth, Severity::Critical), 34);
        assert_eq!(priority(Facility::Local4, Severity::Notice), 165);
        assert_eq!(priority(Facility::Local7, Severity::Debug), 191);
    }
}
