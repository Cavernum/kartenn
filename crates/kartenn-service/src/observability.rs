//! Filters of the technical outputs changed at run time (spec 012, REQ-012-27; 012/L12):
//! an administration operation, audited, valid for a bounded time; every instance applies
//! it and returns to its configuration at expiry. The audit itself has no filter.

use std::future::Future;
use std::time::Duration;

use kartenn_core::audit::{AuditEvent, SYS_LOG_FILTER_CHANGED, SYS_OUTPUT_DISABLED, rfc3339};
use kartenn_core::period::Instant;
use uuid::Uuid;

use crate::audit::{AuditJournal, Outcome, RequestContext, now};
use crate::authz::Authorizer;
use crate::pipeline::RequestError;
use crate::port::StoreError;

/// Permission to change a filter at run time, on [`OBSERVABILITY`].
pub const OBSERVABILITY_ADMIN: &str = "core.observability.admin";

/// The object that stands for the observability settings in authorisations.
pub const OBSERVABILITY: Uuid = Uuid::from_u128(1);

/// Duration of an override when none is given.
pub const DEFAULT_TTL: Duration = Duration::from_hours(1);

/// Longest override.
pub const MAX_TTL: Duration = Duration::from_hours(24);

/// An override in force.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Override {
    /// Output.
    pub output: String,
    /// Filter (`EnvFilter` syntax).
    pub filter: String,
    /// Until.
    pub expires_at: Instant,
}

/// Where the overrides live.
pub trait Overrides: Send + Sync {
    /// Writes the override of `over.output`, with its audit events, in one transaction.
    fn set_override(
        &self,
        over: &Override,
        by: Option<Uuid>,
        events: &[AuditEvent],
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// The overrides not expired yet.
    fn overrides(&self) -> impl Future<Output = Result<Vec<Override>, StoreError>> + Send;
}

/// Sets the filter of `output` to `filter` for `ttl` (1 hour by default, 24 at most), for
/// an actor holding [`OBSERVABILITY_ADMIN`]; `valid` checks the filter syntax (the
/// service knows no filter syntax). A refusal is audited; the filter `off` is also
/// audited as `sys.output.disabled`.
///
/// # Errors
///
/// [`RequestError::Forbidden`] without the permission; [`RequestError::InvalidOverride`]
/// for a filter that does not parse or a duration out of bounds;
/// [`RequestError::Audit`] when the change cannot be audited (it is then not made).
#[allow(clippy::too_many_arguments)] // the context of an administration operation
pub async fn set_log_filter<O: Overrides, A: Authorizer, J: AuditJournal>(
    store: &O,
    authorizer: &A,
    journal: &J,
    context: &RequestContext,
    output: &str,
    filter: &str,
    ttl: Option<Duration>,
    valid: impl Fn(&str) -> bool,
) -> Result<Override, RequestError> {
    let audit_err =
        |e: kartenn_core::audit::AuditError| RequestError::Audit(StoreError(e.to_string()));
    if let Some(actor) = context.subject() {
        let denied = authorizer
            .authorize_many(&actor, OBSERVABILITY_ADMIN, &[OBSERVABILITY])
            .await?;
        if !denied.is_empty() {
            let event = context
                .event(&kartenn_core::audit::AUTHZ_DENIED, Outcome::Denied)
                .map_err(audit_err)?
                .target("observability", [OBSERVABILITY])
                .reason(OBSERVABILITY_ADMIN)
                .detail("permission", OBSERVABILITY_ADMIN)
                .map_err(audit_err)?;
            journal.record(&event).await.map_err(RequestError::Audit)?;
            return Err(RequestError::Forbidden(OBSERVABILITY_ADMIN.into()));
        }
    }
    let ttl = ttl.unwrap_or(DEFAULT_TTL);
    if ttl.is_zero() || ttl > MAX_TTL || !valid(filter) {
        return Err(RequestError::InvalidOverride);
    }
    let expires_at = Instant(now().0 + i64::try_from(ttl.as_micros()).unwrap_or(i64::MAX));
    let over = Override {
        output: output.to_owned(),
        filter: filter.to_owned(),
        expires_at,
    };
    let mut event = context
        .event(&SYS_LOG_FILTER_CHANGED, Outcome::Success)
        .map_err(audit_err)?
        .target("observability", [OBSERVABILITY]);
    for (key, value) in [
        ("output", output.to_owned()),
        ("filter", filter.to_owned()),
        ("expires_at", rfc3339(expires_at)),
    ] {
        event = event.detail(key, value).map_err(audit_err)?;
    }
    let mut events = vec![event];
    // A filter that lets nothing through disables the output (REQ-012-26).
    if filter.trim() == "off" {
        events.push(
            context
                .event(&SYS_OUTPUT_DISABLED, Outcome::Success)
                .map_err(audit_err)?
                .target("observability", [OBSERVABILITY])
                .detail("output", output.to_owned())
                .map_err(audit_err)?,
        );
    }
    store
        .set_override(&over, context.actor.id, &events)
        .await
        .map_err(RequestError::Audit)?;
    Ok(over)
}
