//! Civil dates, Hebcal timestamps and wall-clock time.
//!
//! There is no time-zone database here: every timed Hebcal item carries its
//! own UTC offset (`2027-04-21T19:23:00-04:00`), and the clock uses the POSIX
//! rules in [`crate::tz`].

use core::fmt;

pub const SECS_PER_DAY: i64 = 86_400;

/// A proleptic Gregorian calendar date.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CivilDate {
    pub y: i32,
    pub m: u8,
    pub d: u8,
}

impl CivilDate {
    pub const fn new(y: i32, m: u8, d: u8) -> Self {
        Self { y, m, d }
    }

    /// Days since 1970-01-01 (Howard Hinnant's `days_from_civil`).
    pub const fn days(self) -> i64 {
        let y = if self.m <= 2 { self.y as i64 - 1 } else { self.y as i64 };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let m = self.m as i64;
        let mp = if m > 2 { m - 3 } else { m + 9 };
        let doy = (153 * mp + 2) / 5 + self.d as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// Inverse of [`days`](Self::days) (`civil_from_days`).
    pub const fn from_days(z: i64) -> Self {
        let z = z + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        Self { y: (if m <= 2 { y + 1 } else { y }) as i32, m: m as u8, d: d as u8 }
    }

    pub const fn add_days(self, n: i64) -> Self {
        Self::from_days(self.days() + n)
    }

    pub const fn weekday(self) -> Weekday {
        // 1970-01-01 was a Thursday.
        Weekday::from_index(((self.days() % 7 + 7 + 4) % 7) as u8)
    }

    /// Unix timestamp of local midnight, given the zone offset at that time.
    pub const fn midnight_utc(self, off_secs: i32) -> i64 {
        self.days() * SECS_PER_DAY - off_secs as i64
    }

    /// Parse the leading `YYYY-MM-DD` of `s`.
    pub fn parse(s: &str) -> Option<Self> {
        let b = s.as_bytes();
        if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
            return None;
        }
        let y = s.get(0..4)?.parse().ok()?;
        let m: u8 = s.get(5..7)?.parse().ok()?;
        let d: u8 = s.get(8..10)?.parse().ok()?;
        if !(1..=12).contains(&m) || d == 0 || d > days_in_month(y, m) {
            return None;
        }
        Some(Self::new(y, m, d))
    }
}

impl fmt::Display for CivilDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.y, self.m, self.d)
    }
}

pub const fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub const fn days_in_month(y: i32, m: u8) -> u8 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(y) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Three-letter month abbreviation, `m` in 1..=12.
pub const fn month_short(m: u8) -> &'static str {
    match m {
        1 => "JAN",
        2 => "FEB",
        3 => "MAR",
        4 => "APR",
        5 => "MAY",
        6 => "JUN",
        7 => "JUL",
        8 => "AUG",
        9 => "SEP",
        10 => "OCT",
        11 => "NOV",
        12 => "DEC",
        _ => "???",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Weekday {
    Sun,
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
}

impl Weekday {
    pub const fn from_index(i: u8) -> Self {
        match i % 7 {
            0 => Self::Sun,
            1 => Self::Mon,
            2 => Self::Tue,
            3 => Self::Wed,
            4 => Self::Thu,
            5 => Self::Fri,
            _ => Self::Sat,
        }
    }

    /// 0 = Sunday … 6 = Saturday.
    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn short(self) -> &'static str {
        match self {
            Self::Sun => "SUN",
            Self::Mon => "MON",
            Self::Tue => "TUE",
            Self::Wed => "WED",
            Self::Thu => "THU",
            Self::Fri => "FRI",
            Self::Sat => "SAT",
        }
    }
}

/// An instant plus the UTC offset Hebcal attached to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Stamp {
    pub utc: i64,
    pub off_min: i16,
}

impl Stamp {
    pub const fn local_secs(self) -> i64 {
        self.utc + self.off_min as i64 * 60
    }

    pub const fn local_date(self) -> CivilDate {
        CivilDate::from_days(self.local_secs().div_euclid(SECS_PER_DAY))
    }

    /// Local wall-clock (hour, minute).
    pub const fn hm(self) -> (u8, u8) {
        let s = self.local_secs().rem_euclid(SECS_PER_DAY);
        ((s / 3600) as u8, (s % 3600 / 60) as u8)
    }
}

/// Parse Hebcal's `date` field: `YYYY-MM-DD` (all-day item) or
/// `YYYY-MM-DDTHH:MM[:SS[.fff]]±HH:MM` / `…Z` (timed item).
///
/// A timed value without any zone suffix is taken as UTC.
pub fn parse_hebcal_date(s: &str) -> Option<(CivilDate, Option<Stamp>)> {
    let date = CivilDate::parse(s)?;
    let b = s.as_bytes();
    if b.len() == 10 {
        return Some((date, None));
    }
    if b[10] != b'T' && b[10] != b' ' {
        return None;
    }
    let hh: i64 = s.get(11..13)?.parse().ok()?;
    let mm: i64 = s.get(14..16)?.parse().ok()?;
    let mut i = 16;
    let mut ss = 0i64;
    if b.get(16) == Some(&b':') {
        ss = s.get(17..19)?.parse().ok()?;
        i = 19;
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    }
    let off_min: i64 = match b.get(i) {
        None | Some(b'Z') => 0,
        Some(&sign @ (b'+' | b'-')) => {
            let oh: i64 = s.get(i + 1..i + 3)?.parse().ok()?;
            let om: i64 = match b.get(i + 3) {
                Some(b':') => s.get(i + 4..i + 6)?.parse().ok()?,
                Some(_) => s.get(i + 3..i + 5)?.parse().ok()?,
                None => 0,
            };
            let v = oh * 60 + om;
            if sign == b'-' {
                -v
            } else {
                v
            }
        }
        Some(_) => return None,
    };
    let local = date.days() * SECS_PER_DAY + hh * 3600 + mm * 60 + ss;
    Some((date, Some(Stamp { utc: local - off_min * 60, off_min: off_min as i16 })))
}

/// Wall-clock time at a given UTC offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub date: CivilDate,
    pub h: u8,
    pub m: u8,
    pub s: u8,
    /// Seconds east of UTC.
    pub off_secs: i32,
}

impl LocalTime {
    pub const fn from_utc(utc: i64, off_secs: i32) -> Self {
        let local = utc + off_secs as i64;
        let sod = local.rem_euclid(SECS_PER_DAY);
        Self {
            date: CivilDate::from_days(local.div_euclid(SECS_PER_DAY)),
            h: (sod / 3600) as u8,
            m: (sod % 3600 / 60) as u8,
            s: (sod % 60) as u8,
            off_secs,
        }
    }

    pub const fn secs_of_day(&self) -> u32 {
        self.h as u32 * 3600 + self.m as u32 * 60 + self.s as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_roundtrip_and_weekday() {
        for z in -800_000..800_000i64 {
            if z % 997 != 0 {
                continue;
            }
            assert_eq!(CivilDate::from_days(z).days(), z);
        }
        assert_eq!(CivilDate::new(1970, 1, 1).days(), 0);
        assert_eq!(CivilDate::new(2026, 10, 9).weekday(), Weekday::Fri);
        assert_eq!(CivilDate::new(2027, 4, 22).weekday(), Weekday::Thu);
        assert_eq!(CivilDate::new(2000, 2, 29).add_days(1), CivilDate::new(2000, 3, 1));
    }

    #[test]
    fn hebcal_dates() {
        assert_eq!(parse_hebcal_date("2026-10-04"), Some((CivilDate::new(2026, 10, 4), None)));
        let (d, s) = parse_hebcal_date("2026-10-03T19:15:00-04:00").unwrap();
        let s = s.unwrap();
        assert_eq!(d, CivilDate::new(2026, 10, 3));
        assert_eq!(s.off_min, -240);
        assert_eq!(s.hm(), (19, 15));
        assert_eq!(s.local_date(), d);
        // 19:15 EDT == 23:15 UTC
        assert_eq!(s.utc, CivilDate::new(2026, 10, 3).days() * SECS_PER_DAY + 23 * 3600 + 15 * 60);
        let (_, z) = parse_hebcal_date("2026-10-11T07:22:10.17Z").unwrap();
        assert_eq!(z.unwrap().off_min, 0);
        assert_eq!(parse_hebcal_date("2026-13-01"), None);
        assert_eq!(parse_hebcal_date("nonsense"), None);
    }

    #[test]
    fn local_time() {
        let t = LocalTime::from_utc(-1, 0);
        assert_eq!((t.date, t.h, t.m, t.s), (CivilDate::new(1969, 12, 31), 23, 59, 59));
    }
}
