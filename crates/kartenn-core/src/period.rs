//! Periods of time `[start, end)` (spec 004, REQ-004-13) and the in-memory model of
//! versions split "for a portion of" a period (REQ-004-14, 15).
//!
//! Instants are microseconds since the Unix epoch, UTC: the precision of PostgreSQL
//! `timestamptz`. The version model is the reference against which the storage
//! adapter's splitting is tested (task 004-8).

use std::fmt;

/// An instant, in microseconds since 1970-01-01T00:00:00Z.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant(pub i64);

/// A closed-open period `[start, end)`; `end = None` means unbounded ("in force").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Period {
    start: Instant,
    end: Option<Instant>,
}

/// Errors on periods and versions.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PeriodError {
    /// The period is empty or inverted (REQ-004-13).
    #[error("[{start}, {end}) is empty or inverted")]
    Invalid {
        /// Start, in microseconds.
        start: i64,
        /// End, in microseconds.
        end: i64,
    },
    /// The object has no version over (part of) the targeted period (REQ-004-14).
    #[error("the object does not exist over the whole targeted period")]
    NoVersion,
    /// A creation overlaps an existing version of the object.
    #[error("the object already exists over part of the targeted period")]
    AlreadyExists,
}

impl fmt::Display for Instant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}µs", self.0)
    }
}

impl Period {
    /// A bounded or unbounded period; refuses empty and inverted periods.
    ///
    /// # Errors
    ///
    /// [`PeriodError::Invalid`].
    pub fn new(start: Instant, end: Option<Instant>) -> Result<Self, PeriodError> {
        match end {
            Some(e) if e <= start => Err(PeriodError::Invalid {
                start: start.0,
                end: e.0,
            }),
            _ => Ok(Self { start, end }),
        }
    }

    /// `[start, ∞)`.
    #[must_use]
    pub const fn from(start: Instant) -> Self {
        Self { start, end: None }
    }

    /// Start (included).
    #[must_use]
    pub const fn start(self) -> Instant {
        self.start
    }

    /// End (excluded), `None` if unbounded.
    #[must_use]
    pub const fn end(self) -> Option<Instant> {
        self.end
    }

    /// Whether `t` is in the period.
    #[must_use]
    pub fn contains(self, t: Instant) -> bool {
        self.start <= t && self.end.is_none_or(|e| t < e)
    }

    fn end_le(a: Option<Instant>, b: Option<Instant>) -> bool {
        match (a, b) {
            (_, None) => true,
            (None, Some(_)) => false,
            (Some(x), Some(y)) => x <= y,
        }
    }

    /// Whether `other` is entirely inside this period.
    #[must_use]
    pub fn covers(self, other: Self) -> bool {
        self.start <= other.start && Self::end_le(other.end, self.end)
    }

    /// Whether the periods share an instant.
    #[must_use]
    pub fn overlaps(self, other: Self) -> bool {
        self.intersection(other).is_some()
    }

    /// Common part, if any.
    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        let start = self.start.max(other.start);
        let end = match (self.end, other.end) {
            (None, e) | (e, None) => e,
            (Some(a), Some(b)) => Some(a.min(b)),
        };
        Self::new(start, end).ok()
    }

    /// This period minus `other`: zero, one or two pieces, in order.
    #[must_use]
    pub fn subtract(self, other: Self) -> Vec<Self> {
        if !self.overlaps(other) {
            return vec![self];
        }
        let mut out = Vec::with_capacity(2);
        if self.start < other.start {
            out.extend(Self::new(self.start, Some(other.start)).ok());
        }
        if let Some(other_end) = other.end
            && Self::end_le(Some(other_end), self.end)
            && Some(other_end) != self.end
        {
            out.extend(Self::new(other_end, self.end).ok());
        }
        out
    }

    /// Whether `other` starts exactly where this period ends.
    #[must_use]
    pub fn meets(self, other: Self) -> bool {
        self.end == Some(other.start)
    }
}

/// The versions of one object over application time: sorted, disjoint, without two
/// adjacent versions in the same state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versions<S> {
    versions: Vec<(Period, S)>,
}

impl<S> Default for Versions<S> {
    fn default() -> Self {
        Self {
            versions: Vec::new(),
        }
    }
}

impl<S: Clone + PartialEq> Versions<S> {
    /// The versions, in order.
    #[must_use]
    pub fn as_slice(&self) -> &[(Period, S)] {
        &self.versions
    }

    /// State at `t`, if the object exists then.
    #[must_use]
    pub fn at(&self, t: Instant) -> Option<&S> {
        self.versions
            .iter()
            .find(|(p, _)| p.contains(t))
            .map(|(_, s)| s)
    }

    fn covered(&self, period: Period) -> bool {
        // The versions overlapping `period` must cover it without gap.
        let mut cursor = period.start;
        for (p, _) in self.versions.iter().filter(|(p, _)| p.overlaps(period)) {
            if p.start > cursor {
                return false;
            }
            match p.end {
                None => return true,
                Some(e) => cursor = cursor.max(e),
            }
        }
        period.end.is_some_and(|e| cursor >= e)
    }

    fn normalise(&mut self) {
        self.versions.sort_by_key(|(p, _)| p.start);
        let mut merged: Vec<(Period, S)> = Vec::with_capacity(self.versions.len());
        for (p, s) in self.versions.drain(..) {
            match merged.last_mut() {
                Some((last, ls)) if last.meets(p) && *ls == s => {
                    *last = Period {
                        start: last.start,
                        end: p.end,
                    }
                }
                _ => merged.push((p, s)),
            }
        }
        self.versions = merged;
    }

    /// Creates the object over `period`.
    ///
    /// # Errors
    ///
    /// [`PeriodError::AlreadyExists`] if a version overlaps the period.
    pub fn create(&mut self, period: Period, state: S) -> Result<(), PeriodError> {
        if self.versions.iter().any(|(p, _)| p.overlaps(period)) {
            return Err(PeriodError::AlreadyExists);
        }
        self.versions.push((period, state));
        self.normalise();
        Ok(())
    }

    /// Applies `change` over `portion` only (SQL:2011 `FOR PORTION OF`): versions
    /// crossing the portion are split, their parts outside keep the old state.
    ///
    /// # Errors
    ///
    /// [`PeriodError::NoVersion`] if the object does not exist over the whole portion.
    pub fn update(&mut self, portion: Period, change: impl Fn(&S) -> S) -> Result<(), PeriodError> {
        if !self.covered(portion) {
            return Err(PeriodError::NoVersion);
        }
        let mut next = Vec::with_capacity(self.versions.len() + 2);
        for (p, s) in self.versions.drain(..) {
            match p.intersection(portion) {
                None => next.push((p, s)),
                Some(inside) => {
                    next.extend(
                        p.subtract(portion)
                            .into_iter()
                            .map(|outside| (outside, s.clone())),
                    );
                    next.push((inside, change(&s)));
                }
            }
        }
        self.versions = next;
        self.normalise();
        Ok(())
    }

    /// Ends the object at `at` (REQ-004-15): nothing is erased before `at`.
    ///
    /// # Errors
    ///
    /// [`PeriodError::NoVersion`] if the object does not exist at `at`.
    pub fn close(&mut self, at: Instant) -> Result<(), PeriodError> {
        if self.at(at).is_none() {
            return Err(PeriodError::NoVersion);
        }
        self.versions.retain(|(p, _)| p.start < at);
        for (p, _) in &mut self.versions {
            if p.contains(at) {
                *p = Period {
                    start: p.start,
                    end: Some(at),
                };
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn p(start: i64, end: Option<i64>) -> Period {
        Period::new(Instant(start), end.map(Instant)).unwrap()
    }

    #[test]
    fn periods_are_closed_open_and_non_empty() {
        // REQ-004-13 acceptance.
        assert!(Period::new(Instant(5), Some(Instant(5))).is_err());
        assert!(Period::new(Instant(5), Some(Instant(3))).is_err());
        let a = p(0, Some(10));
        assert!(a.contains(Instant(0)) && !a.contains(Instant(10)));
        assert!(!a.overlaps(p(10, None)), "[0,10) and [10,∞) only meet");
        assert!(a.meets(p(10, None)));
        assert_eq!(
            p(0, Some(10)).subtract(p(3, Some(5))),
            vec![p(0, Some(3)), p(5, Some(10))]
        );
        assert_eq!(
            p(0, None).subtract(p(3, Some(5))),
            vec![p(0, Some(3)), p(5, None)]
        );
        assert_eq!(p(0, Some(10)).subtract(p(0, None)), vec![]);
        assert_eq!(p(0, Some(10)).subtract(p(20, None)), vec![p(0, Some(10))]);
    }

    #[test]
    fn update_for_a_portion_splits_in_three() {
        // REQ-004-14 acceptance: host [jan, ∞), reserved over [mar, jun).
        let (jan, mar, jun) = (0, 59, 151);
        let mut v = Versions::default();
        v.create(p(jan, None), "host").unwrap();
        v.update(p(mar, Some(jun)), |_| "reserved").unwrap();
        assert_eq!(
            v.as_slice(),
            &[
                (p(jan, Some(mar)), "host"),
                (p(mar, Some(jun)), "reserved"),
                (p(jun, None), "host")
            ]
        );
        v.update(p(mar, Some(jun)), |_| "host").unwrap();
        assert_eq!(
            v.as_slice(),
            &[(p(jan, None), "host")],
            "adjacent equal versions merge"
        );
    }

    #[test]
    fn update_outside_existence_is_refused() {
        let mut v = Versions::default();
        v.create(p(10, Some(20)), 1).unwrap();
        assert_eq!(
            v.update(p(15, Some(25)), |s| s + 1),
            Err(PeriodError::NoVersion)
        );
        v.create(p(30, None), 1).unwrap();
        assert_eq!(
            v.update(p(15, Some(35)), |s| s + 1),
            Err(PeriodError::NoVersion),
            "gap 20..30"
        );
        assert_eq!(v.create(p(0, Some(11)), 9), Err(PeriodError::AlreadyExists));
    }

    #[test]
    fn closing_keeps_the_past() {
        // REQ-004-15 acceptance.
        let mut v = Versions::default();
        v.create(p(0, None), "a").unwrap();
        v.update(p(50, None), |_| "b").unwrap();
        v.close(Instant(30)).unwrap();
        assert_eq!(v.as_slice(), &[(p(0, Some(30)), "a")]);
        assert_eq!(v.at(Instant(10)), Some(&"a"));
        assert_eq!(v.at(Instant(40)), None);
        assert_eq!(v.close(Instant(40)), Err(PeriodError::NoVersion));
    }

    fn arb_period() -> impl Strategy<Value = Period> {
        (0i64..100, prop::option::of(1i64..100)).prop_map(|(s, len)| p(s, len.map(|l| s + l)))
    }

    proptest! {
        #[test]
        fn subtract_partitions_the_period(a in arb_period(), b in arb_period()) {
            let pieces = a.subtract(b);
            for t in 0..250 {
                let t = Instant(t);
                let in_pieces = pieces.iter().filter(|x| x.contains(t)).count();
                prop_assert_eq!(in_pieces, usize::from(a.contains(t) && !b.contains(t)));
            }
        }

        #[test]
        fn portion_updates_change_only_the_portion(ops in prop::collection::vec((arb_period(), 0u8..4), 1..8)) {
            // Model: a function instant -> state over [0, 250), compared at every instant.
            let mut v: Versions<u8> = Versions::default();
            v.create(p(0, None), 0).unwrap();
            let mut model = vec![0u8; 250];
            for (portion, value) in ops {
                v.update(portion, |_| value).unwrap();
                for (t, slot) in model.iter_mut().enumerate() {
                    if portion.contains(Instant(i64::try_from(t).unwrap())) {
                        *slot = value;
                    }
                }
                for (t, expected) in model.iter().enumerate() {
                    prop_assert_eq!(v.at(Instant(i64::try_from(t).unwrap())), Some(expected));
                }
                let s = v.as_slice();
                for w in s.windows(2) {
                    prop_assert!(w[0].0.meets(w[1].0) || w[0].0.end() < Some(w[1].0.start()));
                    prop_assert!(!(w[0].0.meets(w[1].0) && w[0].1 == w[1].1), "adjacent equal versions are merged");
                }
            }
        }
    }
}
