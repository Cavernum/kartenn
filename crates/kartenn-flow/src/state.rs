//! Summary of the evaluations of a flow (REQ-010-15), and its state and validity derived
//! without any written state (REQ-011-5, 24 ; design 011 « État dérivé »).

use kartenn_core::period::{Instant, Period};
use kartenn_policy::evaluate::{Outcome, Verdict};

/// Summary of the verdicts of a flow's address pairs (REQ-010-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Summary {
    /// Every pair allowed.
    Allowed,
    /// Some pairs allowed, others not (REQ-011-10).
    Partial,
    /// No pair allowed.
    Denied,
}

/// The summary of a flow's evaluations, with its outcome and counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowSummary {
    /// Allowed, partial or denied.
    pub summary: Summary,
    /// The least favourable outcome (`noncompliant` > `review` > `automatic`).
    pub outcome: Outcome,
    /// Pairs allowed, under reserve or not.
    pub allowed: usize,
    /// Pairs denied.
    pub denied: usize,
}

/// Summarises the verdicts and outcomes of a flow's address pairs. A flow without any
/// pair (endpoints without addresses) allows nothing: denied and not compliant.
#[must_use]
pub fn summarize(pairs: impl IntoIterator<Item = (Verdict, Outcome)>) -> FlowSummary {
    let (mut allowed, mut denied) = (0, 0);
    let mut outcome = Outcome::Automatic;
    for (verdict, pair_outcome) in pairs {
        match verdict {
            Verdict::Allowed | Verdict::AllowedPending => allowed += 1,
            Verdict::Denied => denied += 1,
        }
        outcome = outcome.max(pair_outcome);
    }
    let summary = match (allowed, denied) {
        (0, _) => Summary::Denied,
        (_, 0) => Summary::Allowed,
        _ => Summary::Partial,
    };
    if allowed + denied == 0 {
        outcome = Outcome::Noncompliant;
    }
    FlowSummary {
        summary,
        outcome,
        allowed,
        denied,
    }
}

/// Where the changeset that created a flow stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Creation {
    /// A proposal awaiting review.
    Pending,
    /// A proposal rejected, withdrawn or expired.
    Ended,
    /// Applied.
    Applied,
}

/// The state of a flow at a date (REQ-011-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowState {
    /// Awaiting review.
    Requested,
    /// Rejected at review, withdrawn or expired before review.
    Rejected,
    /// Applied, not started yet.
    Scheduled,
    /// In force and compliant.
    Active,
    /// In force and denied by the policy, without waiver.
    Noncompliant,
    /// In force, denied by the policy and covered by a waiver.
    Waived,
    /// Past its end.
    Expired,
    /// Closed before its end.
    Withdrawn,
}

/// What the state of a flow is derived from, at one date.
#[derive(Debug, Clone, Copy)]
pub struct StateFacts {
    /// Where its creation stands.
    pub creation: Creation,
    /// Its period as last written.
    pub period: Period,
    /// Whether its last version was shortened by a closing.
    pub closed_early: bool,
    /// Its current summary at the date, if evaluated there.
    pub summary: Option<Summary>,
    /// Whether a waiver covers the date.
    pub waived: bool,
}

/// The state of a flow at `t`, in the order of the design: pending creation, ended
/// creation, before its start, after its end, then denied with or without waiver, else
/// active. A flow without end never expires (REQ-011-24).
#[must_use]
pub fn state_at(facts: &StateFacts, t: Instant) -> FlowState {
    match facts.creation {
        Creation::Pending => return FlowState::Requested,
        Creation::Ended => return FlowState::Rejected,
        Creation::Applied => {}
    }
    if t < facts.period.start() {
        return FlowState::Scheduled;
    }
    if let Some(end) = facts.period.end()
        && t >= end
    {
        return if facts.closed_early {
            FlowState::Withdrawn
        } else {
            FlowState::Expired
        };
    }
    match facts.summary {
        Some(Summary::Denied) if facts.waived => FlowState::Waived,
        Some(Summary::Denied) => FlowState::Noncompliant,
        _ => FlowState::Active,
    }
}

/// Whether a flow registered without review holds without end (REQ-011-24): as long as
/// its summary is allowed and its endpoints exist. The module never extends, creates or
/// reclassifies a flow by itself.
#[must_use]
pub fn holds(registered_automatically: bool, summary: Summary, endpoints_exist: bool) -> bool {
    registered_automatically && summary == Summary::Allowed && endpoints_exist
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400_000_000;

    fn at(day: i64) -> Instant {
        Instant(day * DAY)
    }

    fn facts() -> StateFacts {
        StateFacts {
            creation: Creation::Applied,
            period: Period::from(at(10)),
            closed_early: false,
            summary: Some(Summary::Allowed),
            waived: false,
        }
    }

    #[test]
    fn a_pending_proposal_is_requested() {
        let f = StateFacts {
            creation: Creation::Pending,
            ..facts()
        };
        assert_eq!(state_at(&f, at(20)), FlowState::Requested);
    }

    #[test]
    fn a_rejected_proposal_is_rejected() {
        let f = StateFacts {
            creation: Creation::Ended,
            ..facts()
        };
        assert_eq!(state_at(&f, at(20)), FlowState::Rejected);
    }

    #[test]
    fn an_applied_flow_before_its_start_is_scheduled() {
        assert_eq!(state_at(&facts(), at(5)), FlowState::Scheduled);
    }

    #[test]
    fn a_compliant_flow_without_end_stays_active() {
        assert_eq!(state_at(&facts(), at(20)), FlowState::Active);
        assert_eq!(state_at(&facts(), at(100_000)), FlowState::Active);
        assert!(holds(true, Summary::Allowed, true));
        assert!(!holds(true, Summary::Denied, true));
        assert!(!holds(true, Summary::Allowed, false));
        assert!(!holds(false, Summary::Allowed, true));
    }

    #[test]
    fn a_flow_denied_from_a_date_is_noncompliant_or_waived() {
        let denied = StateFacts {
            summary: Some(Summary::Denied),
            ..facts()
        };
        assert_eq!(state_at(&denied, at(20)), FlowState::Noncompliant);
        let waived = StateFacts {
            waived: true,
            ..denied
        };
        assert_eq!(state_at(&waived, at(20)), FlowState::Waived);
        // A partial flow stays active (it went through review at its registration).
        let partial = StateFacts {
            summary: Some(Summary::Partial),
            ..facts()
        };
        assert_eq!(state_at(&partial, at(20)), FlowState::Active);
    }

    #[test]
    fn past_its_end_a_flow_is_expired_or_withdrawn() {
        let bounded = StateFacts {
            period: Period::new(at(10), Some(at(40))).unwrap(),
            ..facts()
        };
        assert_eq!(state_at(&bounded, at(39)), FlowState::Active);
        assert_eq!(state_at(&bounded, at(40)), FlowState::Expired);
        let closed = StateFacts {
            closed_early: true,
            ..bounded
        };
        assert_eq!(state_at(&closed, at(40)), FlowState::Withdrawn);
    }

    #[test]
    fn the_summary_is_allowed_partial_or_denied_with_the_worst_outcome() {
        let all = summarize([
            (Verdict::Allowed, Outcome::Automatic),
            (Verdict::AllowedPending, Outcome::Review),
        ]);
        assert_eq!(
            (all.summary, all.outcome, all.allowed, all.denied),
            (Summary::Allowed, Outcome::Review, 2, 0)
        );
        let partial = summarize([
            (Verdict::Allowed, Outcome::Automatic),
            (Verdict::Denied, Outcome::Noncompliant),
        ]);
        assert_eq!(
            (partial.summary, partial.outcome),
            (Summary::Partial, Outcome::Noncompliant)
        );
        let none = summarize([(Verdict::Denied, Outcome::Noncompliant)]);
        assert_eq!(none.summary, Summary::Denied);
        let empty = summarize([]);
        assert_eq!(
            (empty.summary, empty.outcome),
            (Summary::Denied, Outcome::Noncompliant)
        );
    }
}
