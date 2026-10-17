// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Timestamps: RFC 5424 §6.2.3 (RFC 3339 in UTC with `Z`, six fractional digits) and the
//! legacy `Mmm dd hh:mm:ss` of RFC 3164 §4.1.2. Computed without a calendar library: the
//! civil date of a day count follows H. Hinnant's `civil_from_days`.

use std::time::{SystemTime, UNIX_EPOCH};

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Broken-down UTC time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Utc {
    year: i64,
    month: u32,
    day: u32,
    hour: i64,
    minute: i64,
    second: i64,
    micros: u32,
}

impl Utc {
    /// UTC time of `t`, to the microsecond (truncated).
    pub(crate) fn of(t: SystemTime) -> Self {
        let (secs, micros) = match t.duration_since(UNIX_EPOCH) {
            Ok(d) => (
                i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
                d.subsec_micros(),
            ),
            Err(e) => {
                // Before the epoch: whole seconds rounded down, then the remainder.
                let d = e.duration();
                let secs = -i64::try_from(d.as_secs()).unwrap_or(i64::MAX);
                match d.subsec_micros() {
                    0 => (secs, 0),
                    us => (secs - 1, 1_000_000 - us),
                }
            }
        };
        let (year, month, day) = civil_from_days(secs.div_euclid(86_400));
        let in_day = secs.rem_euclid(86_400);
        Self {
            year,
            month,
            day,
            hour: in_day / 3600,
            minute: in_day % 3600 / 60,
            second: in_day % 60,
            micros,
        }
    }

    /// RFC 3339 with six fractional digits and `Z`; `None` outside years 0 to 9999, which
    /// RFC 3339 cannot write.
    pub(crate) fn rfc3339(self) -> Option<String> {
        if !(0..=9999).contains(&self.year) {
            return None;
        }
        Some(format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second, self.micros
        ))
    }

    /// `Mmm dd hh:mm:ss` (RFC 3164 §4.1.2), the day padded with a space.
    pub(crate) fn rfc3164(self) -> String {
        let month = MONTHS[usize::try_from(self.month - 1).unwrap_or(0)];
        format!(
            "{month} {:>2} {:02}:{:02}:{:02}",
            self.day, self.hour, self.minute, self.second
        )
    }
}

/// Year, month (1 to 12) and day (1 to 31) of the day `days` after 1970-01-01.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    // Both are within 1 to 31 and 1 to 12 by construction.
    (
        year,
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn at(micros: i64) -> Utc {
        let t = if micros >= 0 {
            UNIX_EPOCH + Duration::from_micros(micros.unsigned_abs())
        } else {
            UNIX_EPOCH - Duration::from_micros(micros.unsigned_abs())
        };
        Utc::of(t)
    }

    #[test]
    fn rfc3339_in_utc_to_the_microsecond() {
        let cases = [
            (0, "1970-01-01T00:00:00.000000Z"),
            // RFC 5424 §6.5, example 1: 2003-10-11T22:14:15.003Z.
            (1_065_910_455_003_000, "2003-10-11T22:14:15.003000Z"),
            (-1, "1969-12-31T23:59:59.999999Z"),
            (1_709_164_800_000_001, "2024-02-29T00:00:00.000001Z"),
            (951_782_400_000_000, "2000-02-29T00:00:00.000000Z"),
            (4_107_542_399_999_999, "2100-02-28T23:59:59.999999Z"),
            (253_402_300_799_999_999, "9999-12-31T23:59:59.999999Z"),
        ];
        for (micros, text) in cases {
            assert_eq!(at(micros).rfc3339().as_deref(), Some(text), "{micros}");
        }
        assert_eq!(at(253_402_300_800_000_000).rfc3339(), None, "year 10000");
    }

    #[test]
    fn rfc3164_pads_the_day_with_a_space() {
        assert_eq!(at(1_065_910_455_003_000).rfc3164(), "Oct 11 22:14:15");
        assert_eq!(at(0).rfc3164(), "Jan  1 00:00:00");
    }
}
