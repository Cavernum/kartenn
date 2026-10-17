//! Aggregation of repeated authentication failures (spec 012, REQ-012-19; 012/L13): past a
//! threshold per window and per (source, claimed identifier), failures are counted rather
//! than recorded one by one, and one `authn.login.failures_aggregated` event carries their
//! exact number when the window ends. An attack cannot flood the audit, and nothing is
//! lost from the count.
//!
//! The state is held per instance (D9): each instance emits at most `threshold + 1` events
//! per window and key, and the totals of all instances add up to the exact number.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use kartenn_core::audit::{AUTHN_FAILURES_AGGREGATED, AuditEvent};
use kartenn_core::period::Instant;

/// One window of one key.
#[derive(Debug, Clone)]
struct Window {
    start: Instant,
    seen: u64,
    /// The first event counted beyond the threshold, as a template for the aggregate.
    template: Option<AuditEvent>,
    counted: u64,
}

type Key = (Option<IpAddr>, Option<String>);

/// Aggregates failed authentications.
#[derive(Debug)]
pub struct FailureAggregator {
    threshold: u64,
    window: Duration,
    windows: Mutex<HashMap<Key, Window>>,
}

impl FailureAggregator {
    /// At most `threshold` events per `window` and key; the rest counted.
    #[must_use]
    pub fn new(threshold: u64, window: Duration) -> Self {
        Self {
            threshold,
            window,
            windows: Mutex::new(HashMap::new()),
        }
    }

    fn window_micros(&self) -> i64 {
        i64::try_from(self.window.as_micros()).unwrap_or(i64::MAX)
    }

    /// The aggregate of a window, if it counted anything.
    fn aggregate(&self, window: Window) -> Option<AuditEvent> {
        let template = window.template?;
        let mut event = AuditEvent::new(
            &AUTHN_FAILURES_AGGREGATED,
            template.actor,
            template.outcome,
            window.start,
        )
        .ok()?
        .source(template.source)
        .correlation(template.correlation);
        event = event.detail("count", window.counted).ok()?;
        event.detail("window_seconds", self.window.as_secs()).ok()
    }

    /// A failed authentication `event` at `at`: the events to record now, which are the
    /// event itself while under the threshold, and the aggregate of the previous window of
    /// its key once it is over.
    pub fn failure(&self, event: AuditEvent, at: Instant) -> Vec<AuditEvent> {
        let key = (event.source.ip, event.actor.claimed.clone());
        let mut windows = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
        let mut out = Vec::new();
        let expired = windows
            .get(&key)
            .is_some_and(|w| at.0 - w.start.0 >= self.window_micros());
        if expired && let Some(old) = windows.remove(&key) {
            out.extend(self.aggregate(old));
        }
        let window = windows.entry(key).or_insert_with(|| Window {
            start: at,
            seen: 0,
            template: None,
            counted: 0,
        });
        window.seen += 1;
        if window.seen <= self.threshold {
            out.push(event);
        } else {
            window.counted += 1;
            window.template.get_or_insert(event);
        }
        out
    }

    /// The aggregates of the windows over at `at`, or of every window if `all` (at
    /// shutdown): to call periodically so that a quiet key still reports its count.
    pub fn flush(&self, at: Instant, all: bool) -> Vec<AuditEvent> {
        let mut windows = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
        let over: Vec<Key> = windows
            .iter()
            .filter(|(_, w)| all || at.0 - w.start.0 >= self.window_micros())
            .map(|(k, _)| k.clone())
            .collect();
        over.into_iter()
            .filter_map(|k| windows.remove(&k))
            .filter_map(|w| self.aggregate(w))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use kartenn_core::audit::{AUTHN_LOGIN_FAILED, Actor, ActorKind, Outcome, Source};

    use super::*;

    fn failed(ip: &str, claimed: &str, at: Instant) -> AuditEvent {
        let actor = Actor {
            kind: ActorKind::Unknown,
            id: None,
            auth_method: Some("basic".into()),
            issuer: None,
            claimed: Some(claimed.into()),
        };
        AuditEvent::new(&AUTHN_LOGIN_FAILED, actor, Outcome::Failure, at)
            .unwrap()
            .source(Source {
                ip: Some(ip.parse().unwrap()),
                user_agent: None,
            })
    }

    #[test]
    fn a_thousand_failures_in_a_minute_give_at_most_eleven_events() {
        let aggregator = FailureAggregator::new(10, Duration::from_mins(1));
        let start = 1_000_000_000_000;
        let mut events = Vec::new();
        for i in 0..1000 {
            let at = Instant(start + i * 50_000);
            events.extend(aggregator.failure(failed("192.0.2.7", "h1", at), at));
        }
        // Another source is counted apart.
        let at = Instant(start);
        events.extend(aggregator.failure(failed("198.51.100.1", "h1", at), at));
        events.extend(aggregator.flush(Instant(start + 60_000_000), false));
        let from_first: Vec<&AuditEvent> = events
            .iter()
            .filter(|e| e.source.ip == Some("192.0.2.7".parse().unwrap()))
            .collect();
        assert_eq!(from_first.len(), 11);
        let total: u64 = from_first
            .iter()
            .map(|e| {
                if e.kind.name == "authn.login.failures_aggregated" {
                    e.details["count"].as_u64().unwrap()
                } else {
                    1
                }
            })
            .sum();
        assert_eq!(total, 1000, "the exact number");
        assert_eq!(events.len(), 12);
    }

    #[test]
    fn a_new_window_reports_the_previous_one() {
        let aggregator = FailureAggregator::new(1, Duration::from_mins(1));
        let t = |s: i64| Instant(s * 1_000_000);
        assert_eq!(
            aggregator
                .failure(failed("192.0.2.7", "h", t(0)), t(0))
                .len(),
            1
        );
        assert!(
            aggregator
                .failure(failed("192.0.2.7", "h", t(1)), t(1))
                .is_empty()
        );
        assert!(
            aggregator
                .failure(failed("192.0.2.7", "h", t(2)), t(2))
                .is_empty()
        );
        // Next window: the aggregate of the first (2 counted) and the new failure.
        let next = aggregator.failure(failed("192.0.2.7", "h", t(61)), t(61));
        let names: Vec<&str> = next.iter().map(|e| e.kind.name).collect();
        assert_eq!(
            names,
            ["authn.login.failures_aggregated", "authn.login.failed"]
        );
        assert_eq!(next[0].details["count"], 2);
        // At shutdown, everything still open is reported, nothing counted is lost.
        assert!(
            aggregator
                .failure(failed("192.0.2.7", "h", t(62)), t(62))
                .is_empty()
        );
        assert_eq!(aggregator.flush(t(63), true)[0].details["count"], 1);
    }
}
