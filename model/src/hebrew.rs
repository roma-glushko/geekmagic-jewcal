//! Hebrew calendar arithmetic (the fixed Hillel II calendar).
//!
//! Hebcal supplies holidays and times; this module only names days: today's
//! Hebrew date for the header and the month grid. Tests check it against the
//! `hdate` field Hebcal attaches to every holiday item.
//!
//! Month numbers follow Hebcal: Nisan = 1 … Elul = 6, Tishrei = 7 … Adar I /
//! Adar = 12, Adar II = 13. All integer arithmetic (no floats on Xtensa).

use crate::time::CivilDate;

pub const NISAN: u8 = 1;
pub const IYYAR: u8 = 2;
pub const SIVAN: u8 = 3;
pub const TAMUZ: u8 = 4;
pub const AV: u8 = 5;
pub const ELUL: u8 = 6;
pub const TISHREI: u8 = 7;
pub const CHESHVAN: u8 = 8;
pub const KISLEV: u8 = 9;
pub const ADAR_I: u8 = 12;
pub const ADAR_II: u8 = 13;

/// R.D. (days since 0001-12-31 proleptic Gregorian) offset of Hebcal's epoch.
const EPOCH: i64 = -1_373_428;
/// R.D. of 1970-01-01.
const UNIX_RD: i64 = 719_163;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct HDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

pub const fn is_leap(year: i32) -> bool {
    (7 * year as i64 + 1).rem_euclid(19) < 7
}

pub const fn months_in_year(year: i32) -> u8 {
    if is_leap(year) {
        13
    } else {
        12
    }
}

/// Days from the epoch to 1 Tishrei of `year`, with the postponements.
pub fn elapsed_days(year: i32) -> i64 {
    let y = year as i64 - 1;
    let months = 235 * (y / 19) + 12 * (y % 19) + ((y % 19) * 7 + 1) / 19;
    let parts_elapsed = 204 + 793 * (months % 1080);
    let hours_elapsed = 5 + 12 * months + 793 * (months / 1080) + parts_elapsed / 1080;
    let parts = parts_elapsed % 1080 + 1080 * (hours_elapsed % 24);
    let day = 1 + 29 * months + hours_elapsed / 24;
    let mut alt = day;
    if parts >= 19_440
        || (day % 7 == 2 && parts >= 9_924 && !is_leap(year))
        || (day % 7 == 1 && parts >= 16_789 && is_leap(year - 1))
    {
        alt += 1;
    }
    if matches!(alt % 7, 0 | 3 | 5) {
        alt += 1;
    }
    alt
}

pub fn days_in_year(year: i32) -> i64 {
    elapsed_days(year + 1) - elapsed_days(year)
}

pub fn days_in_month(month: u8, year: i32) -> u8 {
    match month {
        2 | 4 | 6 | 10 | ADAR_II => 29,
        ADAR_I if !is_leap(year) => 29,
        CHESHVAN if days_in_year(year) % 10 != 5 => 29,
        KISLEV if days_in_year(year) % 10 == 3 => 29,
        _ => 30,
    }
}

/// Hebcal's transliteration (`Sh'vat`, `Iyyar`, `Tamuz`, `Adar I`…).
pub fn month_name(month: u8, year: i32) -> &'static str {
    match month {
        1 => "Nisan",
        2 => "Iyyar",
        3 => "Sivan",
        4 => "Tamuz",
        5 => "Av",
        6 => "Elul",
        7 => "Tishrei",
        8 => "Cheshvan",
        9 => "Kislev",
        10 => "Tevet",
        11 => "Sh'vat",
        12 if is_leap(year) => "Adar I",
        12 => "Adar",
        13 => "Adar II",
        _ => "?",
    }
}

fn to_rd(h: HDate) -> i64 {
    let mut days = h.day as i64;
    if h.month < TISHREI {
        for m in TISHREI..=months_in_year(h.year) {
            days += days_in_month(m, h.year) as i64;
        }
        for m in NISAN..h.month {
            days += days_in_month(m, h.year) as i64;
        }
    } else {
        for m in TISHREI..h.month {
            days += days_in_month(m, h.year) as i64;
        }
    }
    EPOCH + elapsed_days(h.year) + days - 1
}

fn new_year_rd(year: i32) -> i64 {
    EPOCH + elapsed_days(year)
}

impl HDate {
    pub fn to_civil(self) -> CivilDate {
        CivilDate::from_days(to_rd(self) - UNIX_RD)
    }

    pub fn from_civil(date: CivilDate) -> HDate {
        let rd = date.days() + UNIX_RD;
        // 35975351 / 98496 is the mean Hebrew year in days; start just below.
        let mut year = ((rd - EPOCH) * 98_496 / 35_975_351) as i32;
        while new_year_rd(year) <= rd {
            year += 1;
        }
        year -= 1;
        let mut month = if rd < to_rd(HDate { year, month: NISAN, day: 1 }) { TISHREI } else { NISAN };
        while rd > to_rd(HDate { year, month, day: days_in_month(month, year) }) {
            month += 1;
        }
        let day = (1 + rd - to_rd(HDate { year, month, day: 1 })) as u8;
        HDate { year, month, day }
    }

    pub fn month_name(self) -> &'static str {
        month_name(self.month, self.year)
    }

    pub fn days_in_month(self) -> u8 {
        days_in_month(self.month, self.year)
    }

    /// Civil date of day 1 of this Hebrew month.
    pub fn month_start(self) -> CivilDate {
        HDate { day: 1, ..self }.to_civil()
    }

    /// Day of the Omer (1..=49) counted on this Hebrew day: 16 Nisan is
    /// day 1, 5 Sivan (Erev Shavuot) day 49.
    pub fn omer(self) -> Option<u8> {
        match self.month {
            NISAN if self.day >= 16 => Some(self.day - 15),
            IYYAR => Some(15 + self.day),
            SIVAN if self.day <= 5 => Some(44 + self.day),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        // Rosh Hashana 5787 = Sat 2026-09-12; Pesach 5787 = Thu 2027-04-22.
        assert_eq!(HDate { year: 5787, month: TISHREI, day: 1 }.to_civil(), CivilDate::new(2026, 9, 12));
        assert_eq!(HDate { year: 5787, month: NISAN, day: 15 }.to_civil(), CivilDate::new(2027, 4, 22));
        assert_eq!(HDate::from_civil(CivilDate::new(2026, 10, 9)), HDate { year: 5787, month: TISHREI, day: 28 });
        assert!(is_leap(5787));
        assert_eq!(days_in_year(5787), 385);
        assert_eq!(month_name(ADAR_I, 5787), "Adar I");
        assert_eq!(month_name(ADAR_I, 5786), "Adar");
        let omer = |m, d| HDate { year: 5787, month: m, day: d }.omer();
        assert_eq!((omer(NISAN, 15), omer(NISAN, 16), omer(IYYAR, 1)), (None, Some(1), Some(16)));
        assert_eq!((omer(IYYAR, 29), omer(SIVAN, 5), omer(SIVAN, 6)), (Some(44), Some(49), None));
    }

    #[test]
    fn roundtrip_over_centuries() {
        let start = CivilDate::new(1900, 1, 1).days();
        let end = CivilDate::new(2200, 1, 1).days();
        let mut d = start;
        while d < end {
            let c = CivilDate::from_days(d);
            let h = HDate::from_civil(c);
            assert!(h.day >= 1 && h.day <= h.days_in_month(), "{c} -> {h:?}");
            assert_eq!(h.to_civil(), c);
            d += 7;
        }
    }
}
