//! Quotas by identity and load shedding (spec 006, REQ-006-29, 30; design « Débit et
//! délestage »).
//!
//! GCRA (`governor`), one keyed limiter per compartment: reads (`r`) and writes (`w`) of
//! an authenticated identity, by its UUID; requests without identity (`a`), by client
//! address, an IPv6 address grouped by /64. Past its quota → 429 with `Retry-After`,
//! `RateLimit-Policy` and `RateLimit` (draft-ietf-httpapi-ratelimit-headers-11).
//!
//! Memory: a limiter holds at most [`Quotas::max_keys`] keys; beyond, the new keys share
//! one overflow key of their compartment (requests without identity in practice: only
//! they can vary their key at will). The idle keys are purged by [`Quotas::tick`], which
//! also returns the refusals aggregated per key and per minute as `abuse.rate.exceeded`
//! events (REQ-012-19): an attack cannot flood the audit, and the count is exact.
//!
//! The limits are per instance (H11): N instances allow N times the quota.

use std::collections::HashMap;
use std::net::IpAddr;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use axum::extract::{Request, State};
use axum::http::{HeaderName, HeaderValue, Method, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use governor::clock::{Clock, DefaultClock};
use governor::{DefaultKeyedRateLimiter, Quota};
use kartenn_core::audit::{Actor, AuditEvent, Outcome, RATE_LIMITED};
use kartenn_core::period::Instant;
use kartenn_service::audit::RequestContext;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::problem::{Problem, ProblemKind};
use crate::transport::Client;

/// A sustained rate and the burst above it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate {
    /// Requests per second, sustained.
    pub per_second: NonZeroU32,
    /// Requests at once.
    pub burst: NonZeroU32,
}

impl Rate {
    /// `per_second` sustained, `burst` at once; `None` if either is zero.
    #[must_use]
    pub fn new(per_second: u32, burst: u32) -> Option<Self> {
        Some(Self {
            per_second: NonZeroU32::new(per_second)?,
            burst: NonZeroU32::new(burst)?,
        })
    }
}

/// The quotas of the compartments (`[http.rate]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rates {
    /// Reads of an authenticated identity.
    pub read: Rate,
    /// Writes of an authenticated identity.
    pub write: Rate,
    /// Requests without identity, by client address.
    pub anonymous: Rate,
}

impl Default for Rates {
    /// The defaults of REQ-006-29: 50/s burst 200, 10/s burst 20, 20/s burst 20.
    fn default() -> Self {
        let rate = |per_second, burst| Rate::new(per_second, burst).expect("non-zero literals");
        Self {
            read: rate(50, 200),
            write: rate(10, 20),
            anonymous: rate(20, 20),
        }
    }
}

/// Most keys held by one limiter, by default.
pub const MAX_KEYS: usize = 100_000;

/// The window over which refusals are aggregated.
const WINDOW: Duration = Duration::from_mins(1);

/// What a limiter counts by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Identity(Uuid),
    Address(IpAddr),
    /// The keys beyond the most a limiter holds.
    Overflow,
}

impl Key {
    /// An address, IPv6 grouped by /64 (one subscriber holds a /64 at least).
    fn address(addr: IpAddr) -> Self {
        Self::Address(match addr.to_canonical() {
            IpAddr::V6(v6) => IpAddr::V6((u128::from(v6) & !u128::from(u64::MAX)).into()),
            v4 @ IpAddr::V4(_) => v4,
        })
    }
}

/// The compartments; the name is that of the policy in the headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Compartment {
    Read,
    Write,
    Anonymous,
}

impl Compartment {
    const fn name(self) -> &'static str {
        match self {
            Self::Read => "r",
            Self::Write => "w",
            Self::Anonymous => "a",
        }
    }
}

struct Limiter {
    rate: Rate,
    keys: DefaultKeyedRateLimiter<Key>,
}

impl Limiter {
    fn new(rate: Rate) -> Self {
        Self {
            rate,
            keys: DefaultKeyedRateLimiter::keyed(
                Quota::per_second(rate.per_second).allow_burst(rate.burst),
            ),
        }
    }
}

/// The refusals of one key in one window, and the first of them as a template.
struct Refusals {
    start: Instant,
    count: u64,
    template: AuditEvent,
}

/// Why a request was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Refused {
    /// The wait before the next token; `None` when the weight exceeds the burst.
    wait: Option<Duration>,
}

/// The limiters of an instance, shared by the requests and the purge timer.
pub struct Quotas {
    read: Limiter,
    write: Limiter,
    anonymous: Limiter,
    max_keys: usize,
    refused: Mutex<HashMap<(Compartment, Key), Refusals>>,
}

impl std::fmt::Debug for Quotas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Quotas")
            .field("max_keys", &self.max_keys)
            .finish_non_exhaustive()
    }
}

impl Quotas {
    /// Limiters at `rates`, of [`MAX_KEYS`] keys each.
    #[must_use]
    pub fn new(rates: Rates) -> Self {
        Self::with_max_keys(rates, MAX_KEYS)
    }

    /// Limiters at `rates`, of `max_keys` keys each (2 at least: one key and the
    /// overflow).
    #[must_use]
    pub fn with_max_keys(rates: Rates, max_keys: usize) -> Self {
        Self {
            read: Limiter::new(rates.read),
            write: Limiter::new(rates.write),
            anonymous: Limiter::new(rates.anonymous),
            max_keys: max_keys.max(2),
            refused: Mutex::new(HashMap::new()),
        }
    }

    const fn limiter(&self, compartment: Compartment) -> &Limiter {
        match compartment {
            Compartment::Read => &self.read,
            Compartment::Write => &self.write,
            Compartment::Anonymous => &self.anonymous,
        }
    }

    /// The keys held by the limiters: reads, writes, without identity.
    #[must_use]
    pub fn keys(&self) -> [usize; 3] {
        [
            self.read.keys.len(),
            self.write.keys.len(),
            self.anonymous.keys.len(),
        ]
    }

    /// Takes `weight` tokens of `key`. A new key past the most held goes to the overflow
    /// key (one entry is kept for it): bounded up to the requests checked at the same
    /// instant.
    fn take(&self, compartment: Compartment, key: Key, weight: NonZeroU32) -> Result<(), Refused> {
        let limiter = &self.limiter(compartment).keys;
        let key = if limiter.len() >= self.max_keys - 1 {
            Key::Overflow
        } else {
            key
        };
        match limiter.check_key_n(&key, weight) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(not_until)) => Err(Refused {
                wait: Some(not_until.wait_time_from(DefaultClock::default().now())),
            }),
            Err(_) => Err(Refused { wait: None }),
        }
    }

    /// Counts a refusal of `key` for the audit.
    fn count(&self, compartment: Compartment, key: Key, context: &RequestContext, client: IpAddr) {
        let at = kartenn_service::audit::now();
        let mut refused = self.refused.lock().unwrap_or_else(PoisonError::into_inner);
        // Bounded like the limiters: beyond, the refusals go to the overflow key.
        let key = if refused.len() >= self.max_keys && !refused.contains_key(&(compartment, key)) {
            Key::Overflow
        } else {
            key
        };
        if let Some(window) = refused.get_mut(&(compartment, key)) {
            window.count += 1;
            return;
        }
        if let Some(template) = template(compartment, context, client, at) {
            refused.insert(
                (compartment, key),
                Refusals {
                    start: at,
                    count: 1,
                    template,
                },
            );
        }
    }

    /// The local timer (every minute): purges the idle keys, and returns the refusals of
    /// the windows over at `at`, or of every window if `all` (at shutdown), to record.
    pub fn tick(&self, at: Instant, all: bool) -> Vec<AuditEvent> {
        for limiter in [&self.read, &self.write, &self.anonymous] {
            limiter.keys.retain_recent();
            limiter.keys.shrink_to_fit();
        }
        let window = i64::try_from(WINDOW.as_micros()).unwrap_or(i64::MAX);
        let mut refused = self.refused.lock().unwrap_or_else(PoisonError::into_inner);
        let over: Vec<_> = refused
            .iter()
            .filter(|(_, w)| all || at.0 - w.start.0 >= window)
            .map(|(k, _)| *k)
            .collect();
        over.into_iter()
            .filter_map(|k| refused.remove(&k))
            .filter_map(|w| {
                w.template
                    .detail("count", w.count)
                    .and_then(|e| e.detail("window_seconds", WINDOW.as_secs()))
                    .ok()
            })
            .collect()
    }
}

/// The event of a refusal: by the identity of the request; without one, by kartenn,
/// which saw the client at `client` (REQ-012-9: no event without an actor).
fn template(
    compartment: Compartment,
    context: &RequestContext,
    client: IpAddr,
    at: Instant,
) -> Option<AuditEvent> {
    let actor = if context.actor.is_attributed() {
        context.actor.clone()
    } else {
        Actor::system()
    };
    let mut source = context.source.clone();
    source.ip.get_or_insert(client);
    AuditEvent::new(&RATE_LIMITED, actor, Outcome::Denied, at)
        .ok()?
        .source(source)
        .correlation(context.correlation.clone())
        .detail("limit", compartment.name())
        .ok()
}

/// Weight of an import chunk of `elements` elements: one per thousand, rounded up.
#[must_use]
pub fn chunk_weight(elements: usize) -> NonZeroU32 {
    NonZeroU32::new(u32::try_from(elements.div_ceil(1000)).unwrap_or(u32::MAX))
        .unwrap_or(NonZeroU32::MIN)
}

/// The quota of a request, in its extensions, for a route whose request weighs more than
/// one (an import chunk).
#[derive(Clone)]
pub struct Charge {
    quotas: Arc<Quotas>,
    compartment: Compartment,
    key: Key,
    context: RequestContext,
    client: IpAddr,
}

impl std::fmt::Debug for Charge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Charge")
            .field("compartment", &self.compartment)
            .finish_non_exhaustive()
    }
}

impl Charge {
    /// Brings the weight of the request to `weight`; the layer took one already.
    ///
    /// # Errors
    ///
    /// The 429 to answer when the quota is exceeded.
    pub fn weigh(&self, weight: NonZeroU32) -> Result<(), TooMany> {
        let Some(more) = NonZeroU32::new(weight.get() - 1) else {
            return Ok(());
        };
        self.quotas
            .take(self.compartment, self.key, more)
            .map_err(|refused| self.refuse(refused))
    }

    fn refuse(&self, refused: Refused) -> TooMany {
        self.quotas
            .count(self.compartment, self.key, &self.context, self.client);
        TooMany {
            compartment: self.compartment,
            rate: self.quotas.limiter(self.compartment).rate,
            refused,
        }
    }
}

/// A request over its quota: the 429.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooMany {
    compartment: Compartment,
    rate: Rate,
    refused: Refused,
}

const RATELIMIT_POLICY: HeaderName = HeaderName::from_static("ratelimit-policy");
const RATELIMIT: HeaderName = HeaderName::from_static("ratelimit");

impl IntoResponse for TooMany {
    /// `Retry-After` is the wait before the next token, rounded up to the second.
    fn into_response(self) -> Response {
        too_many(self.compartment, self.rate, self.refused)
    }
}

fn too_many(compartment: Compartment, rate: Rate, refused: Refused) -> Response {
    let name = compartment.name();
    let policy = format!("\"{name}\";q={};w=1", rate.burst);
    let Some(wait) = refused.wait else {
        return (
            [(RATELIMIT_POLICY, header_value(&policy))],
            Problem::new(ProblemKind::RateLimited)
                .detail("the request weighs more than the quota allows at once"),
        )
            .into_response();
    };
    let seconds = wait.as_secs() + u64::from(wait.subsec_nanos() > 0);
    let seconds = seconds.max(1);
    (
        [
            (header::RETRY_AFTER, HeaderValue::from(seconds)),
            (RATELIMIT_POLICY, header_value(&policy)),
            (
                RATELIMIT,
                header_value(&format!("\"{name}\";r=0;t={seconds}")),
            ),
        ],
        Problem::new(ProblemKind::RateLimited),
    )
        .into_response()
}

fn header_value(text: &str) -> HeaderValue {
    HeaderValue::from_str(text).expect("a policy name and numbers are visible ASCII")
}

/// Methods counted as reads: the safe ones (RFC 9110 §9.2.1), and `QUERY`.
pub(crate) fn reads(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) || method.as_str() == "QUERY"
}

/// Layer 7: one token of the identity of the request (its [`RequestContext`]), or of its
/// client address without one; refused → 429. A [`Charge`] in the extensions for the
/// routes that weigh more. Only `/v1` is counted: the files of the shell and of its
/// plugins, loaded by dozens before any login, are not (spec 007, W18).
pub async fn layer(
    State(quotas): State<Arc<Quotas>>,
    mut request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    // The event stream has ceilings of its own (spec 022, REQ-022-22).
    if !(path == "/v1" || path.starts_with("/v1/")) || path == "/v1/events" {
        return next.run(request).await;
    }
    let context = request
        .extensions()
        .get::<RequestContext>()
        .cloned()
        .unwrap_or_else(RequestContext::system);
    let client = request
        .extensions()
        .get::<Client>()
        .map_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), |c| c.addr);
    let (compartment, key) = match context.actor.id {
        Some(id) if reads(request.method()) => (Compartment::Read, Key::Identity(id)),
        Some(id) => (Compartment::Write, Key::Identity(id)),
        None => (Compartment::Anonymous, Key::address(client)),
    };
    let charge = Charge {
        quotas,
        compartment,
        key,
        context,
        client,
    };
    if let Err(refused) = charge.quotas.take(compartment, key, NonZeroU32::MIN) {
        return charge.refuse(refused).into_response();
    }
    request.extensions_mut().insert(charge);
    next.run(request).await
}

/// The requests in flight of an instance (REQ-006-30).
#[derive(Debug)]
pub struct InFlight(Arc<Semaphore>);

impl InFlight {
    /// At most `max` requests at once (1 at least).
    #[must_use]
    pub fn new(max: usize) -> Self {
        Self(Arc::new(Semaphore::new(max.max(1))))
    }
}

/// Layer 2: at the ceiling of requests in flight → 503 `Retry-After: 1` at once, without
/// waiting nor reaching the route (no queue in kartenn). A permit taken without waiting,
/// as `LoadShed` around `ConcurrencyLimit` would, but one for all the routes: axum
/// layers each route on its own. Only `/v1` is counted, as by the quotas: the ceiling
/// protects the pool, and the files of the shell and of its plugins, loaded by dozens at
/// once, never reach the base; shed, a module would come back as a problem the browser
/// refuses to run.
pub async fn shed(
    State(in_flight): State<Arc<InFlight>>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if !(path == "/v1" || path.starts_with("/v1/")) {
        return next.run(request).await;
    }
    let Ok(_permit) = in_flight.0.clone().try_acquire_owned() else {
        return (
            [(header::RETRY_AFTER, HeaderValue::from_static("1"))],
            Problem::new(ProblemKind::Unavailable).detail("too many requests in flight"),
        )
            .into_response();
    };
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rates(per_second: u32, burst: u32) -> Rates {
        let rate = Rate::new(per_second, burst).unwrap();
        Rates {
            read: rate,
            write: rate,
            anonymous: rate,
        }
    }

    #[test]
    fn ipv6_counts_by_64_and_ipv4_mapped_as_ipv4() {
        let a = Key::address("2001:db8:1:2:aaaa::1".parse().unwrap());
        let b = Key::address("2001:db8:1:2:bbbb::2".parse().unwrap());
        let c = Key::address("2001:db8:1:3::1".parse().unwrap());
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(
            Key::address("::ffff:192.0.2.1".parse().unwrap()),
            Key::address("192.0.2.1".parse().unwrap())
        );
    }

    #[test]
    fn a_chunk_weighs_one_per_thousand_rounded_up() {
        for (elements, weight) in [(0, 1), (1, 1), (1000, 1), (1001, 2), (10_000, 10)] {
            assert_eq!(chunk_weight(elements).get(), weight, "{elements}");
        }
    }

    #[test]
    fn the_weight_counts_and_beyond_the_burst_never_passes() {
        let quotas = Quotas::new(rates(1, 10));
        let key = Key::Identity(Uuid::now_v7());
        let w = |n| NonZeroU32::new(n).unwrap();
        assert!(quotas.take(Compartment::Write, key, w(7)).is_ok());
        let refused = quotas.take(Compartment::Write, key, w(7)).unwrap_err();
        assert!(refused.wait.is_some_and(|d| d > Duration::ZERO));
        assert_eq!(
            quotas.take(Compartment::Write, key, w(11)),
            Err(Refused { wait: None })
        );
        assert!(
            quotas.take(Compartment::Read, key, w(7)).is_ok(),
            "compartments apart"
        );
    }

    #[test]
    fn refusals_are_aggregated_per_key_and_window() {
        let quotas = Quotas::new(rates(1, 1));
        let context = RequestContext::system();
        let client: IpAddr = "192.0.2.1".parse().unwrap();
        let key = Key::address(client);
        let mut unknown = context.clone();
        unknown.actor.kind = kartenn_core::audit::ActorKind::Unknown;
        for _ in 0..1000 {
            quotas.count(Compartment::Anonymous, key, &unknown, client);
        }
        let at = kartenn_service::audit::now();
        assert!(quotas.tick(at, false).is_empty(), "window not over");
        let events = quotas.tick(at, true);
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.kind.name, "abuse.rate.exceeded");
        assert_eq!(event.details["count"], 1000);
        assert_eq!(event.details["limit"], "a");
        assert_eq!(event.source.ip, Some(client));
        assert!(event.actor.is_attributed(), "kartenn when no identity");
    }
}
