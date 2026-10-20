//! VID range of a VLAN group, occupancy in slices and next free VID (REQ-008-12, 13, 20).
//!
//! The storage adapter feeds these functions with the live VLANs of a group at a date;
//! they are the reference for the SQL implementation, as `kartenn_core::occupancy` is for
//! prefixes.

use kartenn_core::rules::Violation;

use crate::rules::VID_OUT_OF_RANGE;
use crate::vlan::{Vid, VlanStatus};

/// The VIDs a group admits, `min` ≤ `max` (V6; 1–4094 by default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VidRange {
    min: Vid,
    max: Vid,
}

impl Default for VidRange {
    fn default() -> Self {
        VidRange {
            min: Vid::MIN,
            max: Vid::MAX,
        }
    }
}

impl VidRange {
    /// Checks a group range: both bounds are VIDs and `min` ≤ `max`.
    ///
    /// # Errors
    /// `vlan.vid.invalid` for a bound outside 1–4094, `vlan.vid.out_of_range` when
    /// `min` > `max`.
    pub fn new(min: i64, max: i64) -> Result<Self, Violation> {
        let (min, max) = (Vid::new(min)?, Vid::new(max)?);
        if min > max {
            return Err(Violation::new(
                &VID_OUT_OF_RANGE,
                format!("plage de VID {min}–{max} vide"),
            ));
        }
        Ok(VidRange { min, max })
    }

    /// Lowest admitted VID.
    #[must_use]
    pub fn min(self) -> Vid {
        self.min
    }

    /// Highest admitted VID.
    #[must_use]
    pub fn max(self) -> Vid {
        self.max
    }

    /// Whether `vid` is admitted.
    #[must_use]
    pub fn contains(self, vid: Vid) -> bool {
        self.min <= vid && vid <= self.max
    }

    /// Number of admitted VIDs.
    #[must_use]
    pub fn len(self) -> u16 {
        self.max.get() - self.min.get() + 1
    }

    /// Never true: a range holds at least one VID.
    #[must_use]
    pub fn is_empty(self) -> bool {
        false
    }

    /// Checks that `vid` is admitted (`vlan.vid.out_of_range`, REQ-008-14).
    ///
    /// # Errors
    /// `vlan.vid.out_of_range` when the VID is outside the range.
    pub fn check(self, vid: Vid) -> Result<(), Violation> {
        if self.contains(vid) {
            Ok(())
        } else {
            Err(Violation::new(
                &VID_OUT_OF_RANGE,
                format!(
                    "VID {vid} hors de la plage du groupe {}–{}",
                    self.min, self.max
                ),
            ))
        }
    }

    /// VIDs among `live` that this range would exclude (REQ-008-13: reducing a range
    /// under a live VLAN is refused and the refusal lists the VLANs), in ascending order.
    #[must_use]
    pub fn excluded(self, live: impl IntoIterator<Item = Vid>) -> Vec<Vid> {
        let mut out: Vec<Vid> = live.into_iter().filter(|&v| !self.contains(v)).collect();
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// State of a VID in a group's occupancy (REQ-008-20).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SliceState {
    /// Carried by an `active` or `deprecated` VLAN.
    Used,
    /// Carried by a `reserved` VLAN.
    Reserved,
    /// Carried by no VLAN.
    Free,
}

impl SliceState {
    fn of(status: VlanStatus) -> Self {
        match status {
            VlanStatus::Active | VlanStatus::Deprecated => SliceState::Used,
            VlanStatus::Reserved => SliceState::Reserved,
        }
    }
}

/// A contiguous run of VIDs in the same state, bounds included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slice {
    /// First VID of the run.
    pub first: Vid,
    /// Last VID of the run.
    pub last: Vid,
    /// State shared by the run.
    pub state: SliceState,
}

/// Occupancy of a group at a date: slices covering its whole range, in order, and
/// counters (REQ-008-20).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupOccupancy {
    /// Contiguous runs, from `min` to `max`.
    pub slices: Vec<Slice>,
    /// VIDs carried by an `active` or `deprecated` VLAN.
    pub used: u16,
    /// VIDs carried by a `reserved` VLAN.
    pub reserved: u16,
    /// VIDs carried by no VLAN.
    pub free: u16,
}

/// Occupancy of `range` given the live VLANs `(vid, status)` of the group, in any order.
/// A VID outside the range is ignored (the range check refuses it elsewhere); a VID
/// listed twice keeps its first state (the uniqueness rule refuses it elsewhere).
#[must_use]
pub fn occupancy(range: VidRange, live: &[(Vid, VlanStatus)]) -> GroupOccupancy {
    let mut taken: Vec<(Vid, SliceState)> = live
        .iter()
        .filter(|(v, _)| range.contains(*v))
        .map(|&(v, s)| (v, SliceState::of(s)))
        .collect();
    taken.sort_by_key(|&(v, _)| v);
    taken.dedup_by_key(|&mut (v, _)| v);

    let mut slices: Vec<Slice> = Vec::new();
    let mut push = |first: Vid, last: Vid, state: SliceState| match slices.last_mut() {
        Some(prev) if prev.state == state && prev.last.succ() == Some(first) => prev.last = last,
        _ => slices.push(Slice { first, last, state }),
    };
    // `cursor` is the first VID not yet covered; `None` once past `max`.
    let mut cursor = Some(range.min());
    for (vid, state) in taken {
        // `c < vid` implies `vid` has a predecessor.
        if let (Some(c), Some(before)) = (cursor.filter(|&c| c < vid), vid.pred()) {
            push(c, before, SliceState::Free);
        }
        push(vid, vid, state);
        cursor = vid.succ().filter(|&n| n <= range.max());
    }
    if let Some(c) = cursor {
        push(c, range.max(), SliceState::Free);
    }

    let count = |state| {
        slices
            .iter()
            .filter(|s| s.state == state)
            .map(|s| s.last.get() - s.first.get() + 1)
            .sum()
    };
    GroupOccupancy {
        used: count(SliceState::Used),
        reserved: count(SliceState::Reserved),
        free: count(SliceState::Free),
        slices,
    }
}

/// Smallest VID of `range`, at or after `from` (the range minimum by default), carried by
/// no live VLAN; `None` when the group is full from there (REQ-008-20, problem
/// `exhausted`).
#[must_use]
pub fn next_free(range: VidRange, live: &[Vid], from: Option<Vid>) -> Option<Vid> {
    let start = from.map_or(range.min(), |f| f.max(range.min()));
    if start > range.max() {
        return None;
    }
    let mut taken: Vec<Vid> = live.iter().copied().filter(|&v| v >= start).collect();
    taken.sort_unstable();
    let mut candidate = start;
    for v in taken {
        match v.cmp(&candidate) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => candidate = v.succ()?,
            std::cmp::Ordering::Greater => break,
        }
    }
    range.contains(candidate).then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vid(v: i64) -> Vid {
        Vid::new(v).unwrap()
    }

    fn range(min: i64, max: i64) -> VidRange {
        VidRange::new(min, max).unwrap()
    }

    #[test]
    fn range_rules() {
        assert_eq!(VidRange::default(), range(1, 4094));
        assert_eq!(
            VidRange::new(0, 10).unwrap_err().rule.id,
            "vlan.vid.invalid"
        );
        assert_eq!(
            VidRange::new(1, 4095).unwrap_err().rule.id,
            "vlan.vid.invalid"
        );
        assert_eq!(
            VidRange::new(10, 9).unwrap_err().rule.id,
            "vlan.vid.out_of_range"
        );
        assert_eq!(range(5, 5).len(), 1);
        let r = range(100, 199);
        assert!(r.check(vid(100)).is_ok());
        assert_eq!(
            r.check(vid(99)).unwrap_err().rule.id,
            "vlan.vid.out_of_range"
        );
        // REQ-008-13: reducing to 1–99 a group holding VLAN 100 lists it.
        assert_eq!(
            range(1, 99).excluded([vid(100), vid(5), vid(100)]),
            vec![vid(100)]
        );
    }

    #[test]
    fn occupancy_slices() {
        // REQ-008-20 acceptance: group 1–10, VIDs 1, 2 active and 5 reserved.
        let live = [
            (vid(5), VlanStatus::Reserved),
            (vid(1), VlanStatus::Active),
            (vid(2), VlanStatus::Active),
        ];
        let occ = occupancy(range(1, 10), &live);
        let got: Vec<_> = occ
            .slices
            .iter()
            .map(|s| (s.first.get(), s.last.get(), s.state))
            .collect();
        assert_eq!(
            got,
            vec![
                (1, 2, SliceState::Used),
                (3, 4, SliceState::Free),
                (5, 5, SliceState::Reserved),
                (6, 10, SliceState::Free),
            ]
        );
        assert_eq!((occ.used, occ.reserved, occ.free), (2, 1, 7));

        // Deprecated counts as used and merges with an adjacent active run; VIDs outside
        // the range are ignored; an empty group is one free slice.
        let occ = occupancy(
            range(10, 12),
            &[
                (vid(11), VlanStatus::Deprecated),
                (vid(12), VlanStatus::Active),
                (vid(3), VlanStatus::Active),
            ],
        );
        assert_eq!(occ.slices.len(), 2);
        assert_eq!((occ.used, occ.reserved, occ.free), (2, 0, 1));
        let occ = occupancy(VidRange::default(), &[]);
        assert_eq!(occ.slices.len(), 1);
        assert_eq!(occ.free, 4094);
        // A VLAN on the last VID of the full range closes the slices without overflow.
        let occ = occupancy(VidRange::default(), &[(Vid::MAX, VlanStatus::Active)]);
        assert_eq!(occ.slices.last().unwrap().state, SliceState::Used);
    }

    #[test]
    fn next_free_from() {
        let live = [vid(1), vid(2), vid(5)];
        let r = range(1, 10);
        assert_eq!(next_free(r, &live, None), Some(vid(3)));
        assert_eq!(next_free(r, &live, Some(vid(5))), Some(vid(6)));
        assert_eq!(next_free(r, &live, Some(vid(4))), Some(vid(4)));
        // A start below the range begins at its minimum; above it, nothing.
        assert_eq!(
            next_free(range(100, 110), &[vid(100)], Some(vid(1))),
            Some(vid(101))
        );
        assert_eq!(next_free(r, &live, Some(vid(11))), None);
    }

    #[test]
    fn exhausted_group() {
        let r = range(1, 3);
        assert_eq!(next_free(r, &[vid(3), vid(1), vid(2)], None), None);
        assert_eq!(next_free(r, &[vid(3)], Some(vid(3))), None);
        assert_eq!(
            next_free(VidRange::default(), &[Vid::MAX], Some(Vid::MAX)),
            None
        );
        let occ = occupancy(
            r,
            &[
                (vid(1), VlanStatus::Active),
                (vid(2), VlanStatus::Reserved),
                (vid(3), VlanStatus::Active),
            ],
        );
        assert_eq!(occ.free, 0);
    }
}
