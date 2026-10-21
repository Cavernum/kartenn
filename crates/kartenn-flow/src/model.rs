//! A requested flow, its endpoints, its waiver and their checks (REQ-011-1 à 4, 9, 20,
//! 22 ; design 011 « Données », « Opérations et règles »).

use kartenn_core::period::{Instant, Period};
use kartenn_core::rules::Violation;
use kartenn_core::values::ExternalRef;
use time::OffsetDateTime;
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

use crate::rules::{
    ENDPOINT_INVALID, FIELD_INVALID, KIND_RESERVED, REFERENCE_MISSING, REVIEW_END_REQUIRED,
    SEPARATION, SERVICE_CLOSED, UPDATE_IDENTITY, WAIVER_INVALID,
};

/// Longest instructed flow, in months, by default (`[flow] review_max_months`, F17).
pub const REVIEW_MAX_MONTHS: u32 = 12;
/// Longest waiver, in months, by default (`[flow] waiver_max_months`, F6).
pub const WAIVER_MAX_MONTHS: u32 = 6;
/// Longest justification, in code points (REQ-011-1).
pub const JUSTIFICATION_MAX: usize = 4_000;
/// Longest identifier of an external reference, in code points (REQ-016-3).
pub const EXTERNAL_ID_MAX: usize = 256;

/// Kind of a flow (REQ-011-20).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowKind {
    /// Requested by someone (v0.2).
    Requested,
    /// Observed on the network: reserved for v0.3, refused in v0.2.
    Observed,
}

/// Kind of object an endpoint designates (REQ-011-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EndpointKind {
    /// An address.
    Address,
    /// A prefix, the node itself (REQ-010-13).
    Prefix,
    /// An interface, where the Machines module is present.
    Interface,
    /// A machine, where the Machines module is present.
    Machine,
    /// A range: never an endpoint (F3).
    Range,
}

/// An endpoint of a flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Endpoint {
    /// What it designates.
    pub kind: EndpointKind,
    /// The object.
    pub id: Uuid,
}

/// On whose behalf a flow is requested (N31): an external reference or a free text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnBehalfOf {
    /// A person known by another system.
    External(ExternalRef),
    /// A name, as given.
    Text(String),
}

/// A flow as requested (REQ-011-1): its identity is its two endpoints and its service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flow {
    /// Requested or observed.
    pub kind: FlowKind,
    /// Source endpoint.
    pub source: Endpoint,
    /// Destination endpoint.
    pub destination: Endpoint,
    /// The service (spec 010).
    pub service: Uuid,
    /// References in other systems, at least one.
    pub external_refs: Vec<ExternalRef>,
    /// Why the flow is needed, if said.
    pub justification: Option<String>,
    /// The identity that requested it.
    pub requester: Uuid,
    /// On whose behalf, if any.
    pub on_behalf_of: Option<OnBehalfOf>,
    /// Owners of the endpoints, if named.
    pub responsibles: Vec<Uuid>,
    /// Values of the sensitivity axis of the data carried.
    pub data_nature: Vec<Uuid>,
    /// Whether personal data travel.
    pub personal_data: bool,
    /// The entity the flow belongs to.
    pub entity: Uuid,
    /// Its period; the end is optional (F4).
    pub period: Period,
}

/// What the checks of a flow need to know of the referential, over the flow's period.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// Whether the Machines module is present (interfaces and machines as endpoints).
    pub machines_present: bool,
    /// Periods over which the source endpoint exists.
    pub source_versions: Vec<Period>,
    /// Periods over which the destination endpoint exists.
    pub destination_versions: Vec<Period>,
    /// Periods over which the service is active.
    pub service_versions: Vec<Period>,
}

/// Whether the union of `versions` covers the whole of `target`.
#[must_use]
pub fn covered(versions: &[Period], target: Period) -> bool {
    let mut rest = vec![target];
    for v in versions {
        rest = rest.into_iter().flat_map(|r| r.subtract(*v)).collect();
        if rest.is_empty() {
            return true;
        }
    }
    rest.is_empty()
}

/// Checks a requested or updated flow (REQ-011-1, 2, 20).
#[must_use]
pub fn check(flow: &Flow, facts: &Facts) -> Vec<Violation> {
    let mut out = Vec::new();
    if flow.kind == FlowKind::Observed {
        out.push(Violation::new(
            &KIND_RESERVED,
            "an observed flow is reserved for v0.3",
        ));
    }
    for (side, endpoint, versions) in [
        ("source", flow.source, &facts.source_versions),
        ("destination", flow.destination, &facts.destination_versions),
    ] {
        let allowed = match endpoint.kind {
            EndpointKind::Address | EndpointKind::Prefix => true,
            EndpointKind::Interface | EndpointKind::Machine => facts.machines_present,
            EndpointKind::Range => false,
        };
        if !allowed {
            out.push(
                Violation::new(
                    &ENDPOINT_INVALID,
                    format!("the {side} is a {:?}, not an endpoint", endpoint.kind),
                )
                .with_subjects([endpoint.id]),
            );
        } else if !covered(versions, flow.period) {
            out.push(
                Violation::new(
                    &ENDPOINT_INVALID,
                    format!("the {side} does not exist over the whole period of the flow"),
                )
                .with_subjects([endpoint.id]),
            );
        }
    }
    if !covered(&facts.service_versions, flow.period) {
        out.push(
            Violation::new(
                &SERVICE_CLOSED,
                "the service is not active over the whole period of the flow",
            )
            .with_subjects([flow.service]),
        );
    }
    if flow.external_refs.is_empty() {
        out.push(Violation::new(
            &REFERENCE_MISSING,
            "a flow carries at least one external reference",
        ));
    }
    for reference in &flow.external_refs {
        if !text_ok(&reference.id, EXTERNAL_ID_MAX) {
            out.push(Violation::new(
                &FIELD_INVALID,
                "an external identifier has 1 to 256 characters, without control character",
            ));
        }
    }
    if let Some(justification) = &flow.justification
        && !text_ok(justification, JUSTIFICATION_MAX)
    {
        out.push(Violation::new(
            &FIELD_INVALID,
            "a justification has 1 to 4 000 characters, without control character",
        ));
    }
    out
}

/// Checks an update of a flow: changing an endpoint or the service creates a new flow
/// (REQ-011-4).
#[must_use]
pub fn check_update(before: &Flow, after: &Flow) -> Vec<Violation> {
    if before.source == after.source
        && before.destination == after.destination
        && before.service == after.service
        && before.kind == after.kind
    {
        Vec::new()
    } else {
        vec![Violation::new(
            &UPDATE_IDENTITY,
            "changing an endpoint, the service or the kind of a flow creates a new flow",
        )]
    }
}

/// A waiver of a denied flow (REQ-011-9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiver {
    /// Why the flow is accepted despite the policy.
    pub reason: String,
    /// The risk accepted.
    pub accepted_risk: String,
    /// Who granted it.
    pub granted_by: Uuid,
    /// When.
    pub granted_at: Instant,
    /// Its end, mandatory.
    pub end: Option<Instant>,
}

/// Checks a waiver of a flow of `requester` ending at `flow_end` (REQ-011-9): reason and
/// accepted risk not empty, an end at the earliest of the flow's end and the grant plus
/// `max_months`, and a grantor other than the requester.
#[must_use]
pub fn check_waiver(
    waiver: &Waiver,
    requester: Uuid,
    flow_end: Option<Instant>,
    max_months: u32,
) -> Vec<Violation> {
    let mut out = Vec::new();
    let mut invalid = |why: &str| out.push(Violation::new(&WAIVER_INVALID, why.to_owned()));
    if !text_ok(&waiver.reason, JUSTIFICATION_MAX) {
        invalid("a waiver has a reason");
    }
    if !text_ok(&waiver.accepted_risk, JUSTIFICATION_MAX) {
        invalid("a waiver names the risk accepted");
    }
    if waiver.granted_by == requester {
        invalid("the requester of a flow does not grant its waiver");
    }
    match waiver.end {
        None => invalid("a waiver has an end"),
        Some(end) => {
            let ceiling = add_months(waiver.granted_at, max_months);
            let limit = flow_end.map_or(ceiling, |f| f.min(ceiling));
            if end > limit {
                invalid("a waiver ends at the latest at the end of the flow and its ceiling");
            }
            if end <= waiver.granted_at {
                invalid("a waiver ends after it is granted");
            }
        }
    }
    out
}

/// Checks the approval of an instructed request (REQ-011-22): a reviewer other than the
/// requester, and an end to the flow within `max_months` of the approval.
#[must_use]
pub fn check_review(
    reviewer: Uuid,
    requester: Uuid,
    approved_at: Instant,
    flow_end: Option<Instant>,
    max_months: u32,
) -> Vec<Violation> {
    let mut out = Vec::new();
    if reviewer == requester {
        out.push(Violation::new(
            &SEPARATION,
            "the requester of a flow does not review it",
        ));
    }
    match flow_end {
        Some(end) if end <= add_months(approved_at, max_months) => {}
        Some(_) => out.push(Violation::new(
            &REVIEW_END_REQUIRED,
            format!("an instructed flow ends within {max_months} months of its approval"),
        )),
        None => out.push(Violation::new(
            &REVIEW_END_REQUIRED,
            "an instructed flow has an end",
        )),
    }
    out
}

/// `at` plus `months` calendar months, the day clamped to the end of the month, in UTC.
#[must_use]
pub fn add_months(at: Instant, months: u32) -> Instant {
    let Ok(t) = OffsetDateTime::from_unix_timestamp_nanos(i128::from(at.0) * 1_000) else {
        return at;
    };
    let total = i64::from(t.month() as u8) - 1 + i64::from(months);
    let year = t.year() + i32::try_from(total.div_euclid(12)).unwrap_or(0);
    let Ok(month) = time::Month::try_from(u8::try_from(total.rem_euclid(12) + 1).unwrap_or(1))
    else {
        return at;
    };
    let day = t.day().min(time::util::days_in_month(month, year));
    let Ok(date) = time::Date::from_calendar_date(year, month, day) else {
        return at;
    };
    let shifted = date.with_time(t.time()).assume_utc();
    let micros = (shifted - OffsetDateTime::UNIX_EPOCH).whole_microseconds();
    i64::try_from(micros).map_or(at, Instant)
}

/// Text of 1 to `max` code points once in NFC, without control character other than a
/// line break.
fn text_ok(text: &str, max: usize) -> bool {
    let normal: String = text.nfc().collect();
    let len = normal.chars().count();
    (1..=max).contains(&len)
        && !normal.trim().is_empty()
        && !normal.chars().any(|c| c.is_control() && c != '\n')
}

/// A day, in microseconds: for tests and callers that count days.
pub const DAY: i64 = 86_400_000_000;

#[cfg(test)]
mod tests {
    use super::*;

    const T0: Instant = Instant(1_767_225_600_000_000); // 2026-01-01T00:00:00Z

    fn days(n: i64) -> Instant {
        Instant(T0.0 + n * DAY)
    }

    fn reference() -> ExternalRef {
        ExternalRef {
            system: Uuid::from_u128(1),
            id: "CHG-1042".into(),
            label: None,
        }
    }

    fn flow() -> Flow {
        Flow {
            kind: FlowKind::Requested,
            source: Endpoint {
                kind: EndpointKind::Machine,
                id: Uuid::from_u128(10),
            },
            destination: Endpoint {
                kind: EndpointKind::Prefix,
                id: Uuid::from_u128(11),
            },
            service: Uuid::from_u128(20),
            external_refs: vec![reference()],
            justification: None,
            requester: Uuid::from_u128(30),
            on_behalf_of: None,
            responsibles: Vec::new(),
            data_nature: Vec::new(),
            personal_data: false,
            entity: Uuid::from_u128(40),
            period: Period::from(T0),
        }
    }

    fn facts() -> Facts {
        Facts {
            machines_present: true,
            source_versions: vec![Period::from(days(-10))],
            destination_versions: vec![Period::from(days(-10))],
            service_versions: vec![Period::from(days(-10))],
        }
    }

    fn ids(violations: &[Violation]) -> Vec<&'static str> {
        violations.iter().map(|v| v.rule.id).collect()
    }

    #[test]
    fn a_flow_without_justification_nor_end_with_a_reference_is_accepted() {
        assert!(check(&flow(), &facts()).is_empty());
    }

    #[test]
    fn a_flow_without_external_reference_is_refused() {
        let mut f = flow();
        f.external_refs.clear();
        assert_eq!(ids(&check(&f, &facts())), ["flow.reference.missing"]);
    }

    #[test]
    fn a_service_closed_before_the_end_of_the_flow_is_refused() {
        let mut facts = facts();
        facts.service_versions = vec![Period::new(days(-10), Some(days(151))).unwrap()];
        assert_eq!(ids(&check(&flow(), &facts)), ["flow.service.closed"]);
        let mut bounded = flow();
        bounded.period = Period::new(T0, Some(days(100))).unwrap();
        assert!(check(&bounded, &facts).is_empty());
    }

    #[test]
    fn a_range_or_a_machine_without_the_module_is_not_an_endpoint() {
        let mut f = flow();
        f.destination.kind = EndpointKind::Range;
        assert_eq!(ids(&check(&f, &facts())), ["flow.endpoint.invalid"]);
        let mut no_machines = facts();
        no_machines.machines_present = false;
        assert_eq!(
            ids(&check(&flow(), &no_machines)),
            ["flow.endpoint.invalid"]
        );
    }

    #[test]
    fn an_endpoint_closed_during_the_flow_is_refused() {
        let mut facts = facts();
        facts.source_versions = vec![Period::new(days(-10), Some(days(5))).unwrap()];
        assert_eq!(ids(&check(&flow(), &facts)), ["flow.endpoint.invalid"]);
        // Two consecutive versions cover the period.
        facts.source_versions.push(Period::from(days(5)));
        assert!(check(&flow(), &facts).is_empty());
    }

    #[test]
    fn an_observed_flow_is_reserved() {
        let mut f = flow();
        f.kind = FlowKind::Observed;
        assert_eq!(ids(&check(&f, &facts())), ["flow.kind.reserved"]);
    }

    #[test]
    fn a_justification_too_long_or_blank_is_refused() {
        let mut f = flow();
        f.justification = Some("x".repeat(4_001));
        assert_eq!(ids(&check(&f, &facts())), ["flow.field.invalid"]);
        f.justification = Some("  ".into());
        assert_eq!(ids(&check(&f, &facts())), ["flow.field.invalid"]);
        f.justification = Some("x".repeat(4_000));
        assert!(check(&f, &facts()).is_empty());
    }

    #[test]
    fn changing_an_endpoint_or_the_service_is_a_new_flow() {
        let before = flow();
        let mut after = flow();
        after.justification = Some("raison".into());
        assert!(check_update(&before, &after).is_empty());
        after.service = Uuid::from_u128(21);
        assert_eq!(
            ids(&check_update(&before, &after)),
            ["flow.update.identity"]
        );
    }

    fn waiver() -> Waiver {
        Waiver {
            reason: "migration en cours".into(),
            accepted_risk: "accès administrateur temporaire".into(),
            granted_by: Uuid::from_u128(31),
            granted_at: T0,
            end: Some(days(90)),
        }
    }

    #[test]
    fn a_waiver_with_reason_risk_and_end_by_another_is_accepted() {
        let requester = Uuid::from_u128(30);
        assert!(check_waiver(&waiver(), requester, None, WAIVER_MAX_MONTHS).is_empty());
    }

    #[test]
    fn a_waiver_without_risk_beyond_six_months_or_by_the_requester_is_refused() {
        let requester = Uuid::from_u128(30);
        let mut w = waiver();
        w.accepted_risk = String::new();
        assert_eq!(
            ids(&check_waiver(&w, requester, None, WAIVER_MAX_MONTHS)),
            ["flow.waiver.invalid"]
        );
        let mut w = waiver();
        w.end = Some(add_months(T0, 6));
        assert!(check_waiver(&w, requester, None, WAIVER_MAX_MONTHS).is_empty());
        w.end = Some(Instant(add_months(T0, 6).0 + 1));
        assert_eq!(
            ids(&check_waiver(&w, requester, None, WAIVER_MAX_MONTHS)),
            ["flow.waiver.invalid"]
        );
        let mut w = waiver();
        w.granted_by = requester;
        assert_eq!(
            ids(&check_waiver(&w, requester, None, WAIVER_MAX_MONTHS)),
            ["flow.waiver.invalid"]
        );
        // Beyond the end of the flow.
        assert_eq!(
            ids(&check_waiver(
                &waiver(),
                requester,
                Some(days(30)),
                WAIVER_MAX_MONTHS
            )),
            ["flow.waiver.invalid"]
        );
        let mut w = waiver();
        w.end = None;
        assert_eq!(
            ids(&check_waiver(&w, requester, None, WAIVER_MAX_MONTHS)),
            ["flow.waiver.invalid"]
        );
    }

    #[test]
    fn the_requester_does_not_review_and_an_instructed_flow_ends() {
        let (requester, reviewer) = (Uuid::from_u128(30), Uuid::from_u128(32));
        let end = Some(add_months(T0, 12));
        assert!(check_review(reviewer, requester, T0, end, REVIEW_MAX_MONTHS).is_empty());
        assert_eq!(
            ids(&check_review(
                requester,
                requester,
                T0,
                end,
                REVIEW_MAX_MONTHS
            )),
            ["flow.separation"]
        );
        assert_eq!(
            ids(&check_review(
                reviewer,
                requester,
                T0,
                None,
                REVIEW_MAX_MONTHS
            )),
            ["flow.review.end_required"]
        );
        assert_eq!(
            ids(&check_review(
                reviewer,
                requester,
                T0,
                Some(add_months(T0, 13)),
                REVIEW_MAX_MONTHS
            )),
            ["flow.review.end_required"]
        );
    }

    #[test]
    fn months_are_calendar_months_clamped_to_the_end_of_the_month() {
        // 2026-01-31 + 1 month = 2026-02-28.
        let jan31 = days(30);
        assert_eq!(add_months(jan31, 1), days(30 + 28));
        // 2026-01-01 + 12 months = 2027-01-01.
        assert_eq!(add_months(T0, 12), days(365));
    }
}
