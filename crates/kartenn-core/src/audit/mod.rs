//! Audit events (spec 012): who did what, when, on what, with which outcome. Events are
//! typed here and written by the port of `kartenn-service`; the catalogue lists every
//! kind, the canonical form fingerprints them for the seals.

mod catalogue;
pub mod jcs;
pub mod seal;

use std::fmt;
use std::net::IpAddr;

use serde_json::{Map, Value, json};
use uuid::Uuid;

pub use catalogue::*;

use crate::period::Instant;

/// Severity of an event, for audit and technical logs alike (REQ-012-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Detail for developers.
    Trace,
    /// Diagnosis.
    Debug,
    /// Normal operation.
    Info,
    /// Unexpected but handled.
    Warn,
    /// Failure.
    Error,
}

impl Severity {
    /// Name, in capitals.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }

    /// Syslog severity code (RFC 5424 §6.2.1), the table of REQ-012-4.
    #[must_use]
    pub const fn syslog(self) -> u8 {
        match self {
            Self::Trace | Self::Debug => 7,
            Self::Info => 6,
            Self::Warn => 4,
            Self::Error => 3,
        }
    }

    /// OpenTelemetry `SeverityNumber`, the table of REQ-012-4.
    #[must_use]
    pub const fn otel(self) -> u8 {
        match self {
            Self::Trace => 1,
            Self::Debug => 5,
            Self::Info => 9,
            Self::Warn => 13,
            Self::Error => 17,
        }
    }

    /// journald `PRIORITY`: the syslog code.
    #[must_use]
    pub const fn journald(self) -> u8 {
        self.syslog()
    }
}

/// How long an audit event must be kept (012/L22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Criticality {
    /// Ordinary reads.
    Low,
    /// Writes and operation.
    Normal,
    /// Refusals, administration, sensitive reads.
    High,
}

impl Criticality {
    /// Name as stored and exported.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
        }
    }
}

/// Kind of actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActorKind {
    /// A person, by internal identifier (004/C4).
    Person,
    /// A service account.
    ServiceAccount,
    /// kartenn itself (jobs, migrations).
    System,
    /// Not established: a failed authentication.
    Unknown,
}

impl ActorKind {
    /// Name as stored and exported.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::ServiceAccount => "service_account",
            Self::System => "system",
            Self::Unknown => "unknown",
        }
    }
}

/// Who acted (REQ-012-9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    /// Kind.
    pub kind: ActorKind,
    /// Internal identifier, for a person or a service account.
    pub id: Option<Uuid>,
    /// Authentication method (`oidc`, `basic`…).
    pub auth_method: Option<String>,
    /// Issuer of the identity.
    pub issuer: Option<String>,
    /// HMAC of the identifier claimed by an unknown actor, in hexadecimal; never the
    /// identifier itself.
    pub claimed: Option<String>,
}

impl Actor {
    /// kartenn itself.
    #[must_use]
    pub const fn system() -> Self {
        Self {
            kind: ActorKind::System,
            id: None,
            auth_method: None,
            issuer: None,
            claimed: None,
        }
    }

    /// A person, by internal identifier.
    #[must_use]
    pub const fn person(id: Uuid) -> Self {
        Self {
            kind: ActorKind::Person,
            id: Some(id),
            auth_method: None,
            issuer: None,
            claimed: None,
        }
    }

    /// Whether the actor is established, as every event but a failed authentication
    /// requires (REQ-012-9).
    #[must_use]
    pub fn is_attributed(&self) -> bool {
        match self.kind {
            ActorKind::System => true,
            ActorKind::Person | ActorKind::ServiceAccount => self.id.is_some(),
            ActorKind::Unknown => self.claimed.is_some(),
        }
    }
}

/// Outcome of the action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Outcome {
    /// Done.
    Success,
    /// Failed (invalid, unavailable).
    Failure,
    /// Refused by a right.
    Denied,
}

impl Outcome {
    /// Name as stored and exported.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Denied => "denied",
        }
    }
}

/// Where the request came from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Source {
    /// Client address, after the trusted proxies.
    pub ip: Option<IpAddr>,
    /// User agent, at most [`USER_AGENT_MAX`] octets.
    pub user_agent: Option<String>,
}

/// Longest user agent kept.
pub const USER_AGENT_MAX: usize = 256;

/// Correlation with traces and requests (REQ-012-6).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Correlation {
    /// W3C trace identifier, 32 hexadecimal digits.
    pub trace_id: Option<String>,
    /// W3C span identifier, 16 hexadecimal digits.
    pub span_id: Option<String>,
    /// Identifier of the request, returned to the client.
    pub request_id: Option<String>,
}

/// What the action was about: a kind of object and identifiers, never values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// Kind of object (`prefix`, `changeset`, `property`…).
    pub kind: String,
    /// Identifiers.
    pub ids: Vec<Uuid>,
}

/// Why an event cannot be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuditError {
    /// A detail the catalogue does not declare for this event.
    #[error("{event} declares no detail {key:?}")]
    UnknownDetail {
        /// Event.
        event: &'static str,
        /// Detail key.
        key: String,
    },
    /// No actor established (REQ-012-9).
    #[error("{0} has no actor")]
    Unattributed(&'static str),
}

/// An audit event (REQ-012-10).
#[derive(Debug, Clone, PartialEq)]
pub struct AuditEvent {
    /// Identifier (UUID v7).
    pub id: Uuid,
    /// Kind, from the catalogue.
    pub kind: &'static EventKind,
    /// Time of the action, on the clock of the instance.
    pub occurred_at: Instant,
    /// Who.
    pub actor: Actor,
    /// From where.
    pub source: Source,
    /// On what.
    pub target: Option<Target>,
    /// Result.
    pub outcome: Outcome,
    /// Reason code (rule identifier, permission…).
    pub reason: Option<String>,
    /// Motive given for the action.
    pub motive: Option<String>,
    /// Changeset concerned.
    pub changeset: Option<Uuid>,
    /// Correlation.
    pub correlation: Correlation,
    /// Details declared by the catalogue for this kind.
    pub details: Map<String, Value>,
}

impl AuditEvent {
    /// An event of `kind` by `actor` at `occurred_at`.
    ///
    /// # Errors
    ///
    /// [`AuditError::Unattributed`] when the actor is not established.
    pub fn new(
        kind: &'static EventKind,
        actor: Actor,
        outcome: Outcome,
        occurred_at: Instant,
    ) -> Result<Self, AuditError> {
        if !actor.is_attributed() {
            return Err(AuditError::Unattributed(kind.name));
        }
        Ok(Self {
            id: Uuid::now_v7(),
            kind,
            occurred_at,
            actor,
            source: Source::default(),
            target: None,
            outcome,
            reason: None,
            motive: None,
            changeset: None,
            correlation: Correlation::default(),
            details: Map::new(),
        })
    }

    /// Adds a detail declared by the catalogue.
    ///
    /// # Errors
    ///
    /// [`AuditError::UnknownDetail`] for a key the kind does not declare.
    pub fn detail(mut self, key: &str, value: impl Into<Value>) -> Result<Self, AuditError> {
        if !self.kind.details.contains(&key) {
            return Err(AuditError::UnknownDetail {
                event: self.kind.name,
                key: key.to_owned(),
            });
        }
        self.details.insert(key.to_owned(), value.into());
        Ok(self)
    }

    /// Sets the target.
    #[must_use]
    pub fn target(mut self, kind: impl Into<String>, ids: impl IntoIterator<Item = Uuid>) -> Self {
        self.target = Some(Target {
            kind: kind.into(),
            ids: ids.into_iter().collect(),
        });
        self
    }

    /// Sets the source, the user agent cut to [`USER_AGENT_MAX`] octets.
    #[must_use]
    pub fn source(mut self, mut source: Source) -> Self {
        if let Some(agent) = &mut source.user_agent {
            let mut end = agent.len().min(USER_AGENT_MAX);
            while !agent.is_char_boundary(end) {
                end -= 1;
            }
            agent.truncate(end);
        }
        self.source = source;
        self
    }

    /// Sets the reason code.
    #[must_use]
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Sets the motive.
    #[must_use]
    pub fn motive(mut self, motive: impl Into<String>) -> Self {
        self.motive = Some(motive.into());
        self
    }

    /// Sets the changeset.
    #[must_use]
    pub const fn changeset(mut self, changeset: Uuid) -> Self {
        self.changeset = Some(changeset);
        self
    }

    /// Sets the correlation.
    #[must_use]
    pub fn correlation(mut self, correlation: Correlation) -> Self {
        self.correlation = correlation;
        self
    }

    /// The JSON form, as exported (REQ-012-10).
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "name": self.kind.name,
            "severity": self.kind.severity.as_str(),
            "criticality": self.kind.criticality.as_str(),
            "occurred_at": rfc3339(self.occurred_at),
            "actor": {
                "kind": self.actor.kind.as_str(),
                "id": self.actor.id,
                "auth_method": self.actor.auth_method,
                "issuer": self.actor.issuer,
                "claimed": self.actor.claimed,
            },
            "source": {
                "ip": self.source.ip.map(|ip| ip.to_string()),
                "user_agent": self.source.user_agent,
            },
            "target": self.target.as_ref().map(|t| json!({"kind": t.kind, "ids": t.ids})),
            "outcome": self.outcome.as_str(),
            "reason": self.reason,
            "motive": self.motive,
            "changeset": self.changeset,
            "trace_id": self.correlation.trace_id,
            "span_id": self.correlation.span_id,
            "request_id": self.correlation.request_id,
            "details": self.details,
        })
    }
}

impl fmt::Display for AuditEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {}",
            self.kind.name,
            self.outcome.as_str(),
            self.id
        )
    }
}

/// RFC 3339 in UTC, six fractional digits (REQ-012-3).
#[must_use]
pub fn rfc3339(at: Instant) -> String {
    let micros = at.0.rem_euclid(1_000_000);
    let secs = at.0.div_euclid(1_000_000);
    let days = secs.div_euclid(86_400);
    let in_day = secs.rem_euclid(86_400);
    // H. Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{micros:06}Z",
        in_day / 3600,
        in_day % 3600 / 60,
        in_day % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severities_follow_the_table_of_req_012_4() {
        let table = [
            (Severity::Error, 3, 17),
            (Severity::Warn, 4, 13),
            (Severity::Info, 6, 9),
            (Severity::Debug, 7, 5),
            (Severity::Trace, 7, 1),
        ];
        for (s, syslog, otel) in table {
            assert_eq!((s.syslog(), s.otel(), s.journald()), (syslog, otel, syslog));
        }
    }

    #[test]
    fn an_event_is_attributed_and_declares_its_details() {
        let at = Instant(1_065_910_455_003_000);
        let unknown = Actor {
            kind: ActorKind::Unknown,
            id: None,
            auth_method: Some("basic".into()),
            issuer: None,
            claimed: None,
        };
        assert_eq!(
            AuditEvent::new(&AUTHN_LOGIN_FAILED, unknown.clone(), Outcome::Failure, at),
            Err(AuditError::Unattributed("authn.login.failed"))
        );
        let claimed = Actor {
            claimed: Some("9f86d081".into()),
            ..unknown
        };
        assert!(AuditEvent::new(&AUTHN_LOGIN_FAILED, claimed, Outcome::Failure, at).is_ok());

        let person = Uuid::now_v7();
        let event = AuditEvent::new(&AUTHZ_DENIED, Actor::person(person), Outcome::Denied, at)
            .unwrap()
            .detail("permission", "prop.custom.env.write")
            .unwrap()
            .target("prefix", [Uuid::nil()])
            .source(Source {
                ip: Some("192.0.2.7".parse().unwrap()),
                user_agent: Some("é".repeat(200)),
            });
        assert!(event.source.user_agent.as_ref().unwrap().len() <= USER_AGENT_MAX);
        assert!(matches!(
            event.clone().detail("password", "x"),
            Err(AuditError::UnknownDetail { .. })
        ));
        let json = event.to_json();
        assert_eq!(json["name"], "authz.access.denied");
        assert_eq!(json["criticality"], "high");
        assert_eq!(json["occurred_at"], "2003-10-11T22:14:15.003000Z");
        assert_eq!(json["actor"]["id"], person.to_string());
        assert_eq!(json["details"]["permission"], "prop.custom.env.write");
        assert!(jcs::canonical(&json).is_ok());
    }

    #[test]
    fn rfc3339_before_and_after_the_epoch() {
        assert_eq!(rfc3339(Instant(0)), "1970-01-01T00:00:00.000000Z");
        assert_eq!(rfc3339(Instant(-1)), "1969-12-31T23:59:59.999999Z");
        assert_eq!(
            rfc3339(Instant(1_709_164_800_000_001)),
            "2024-02-29T00:00:00.000001Z"
        );
    }
}
