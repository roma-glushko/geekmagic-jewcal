//! Minimal POSIX TZ evaluator: the rule format found in TZif footers, e.g.
//! `EST5EDT,M3.2.0,M11.1.0` or Jerusalem's `IST-2IDT,M3.4.4/26,M10.5.0`.
//!
//! Supports quoted names (`<-03>3`), offsets with minutes/seconds, `Mm.w.d`,
//! `Jn` and `n` rule days, and rule times outside 0–24 h (RFC 8536 allows
//! −167…167 h; Jerusalem starts DST at "Thursday 26:00" = Friday 02:00).
//! The same code runs on the host and the ESP32, so tests cover the device.

use crate::time::{days_in_month, is_leap, CivilDate, LocalTime, SECS_PER_DAY};

/// A zone's rules. Offsets are seconds **east** of UTC.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tz {
    std_off: i32,
    dst: Option<Dst>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Dst {
    off: i32,
    start: Rule,
    end: Rule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rule {
    day: RuleDay,
    /// Local time of the transition, seconds (may be negative or > 24 h).
    time: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuleDay {
    /// `Mm.w.d`: day `d` (0 = Sunday) of week `w` (5 = last) of month `m`.
    Month { m: u8, w: u8, d: u8 },
    /// `Jn`: 1…365, February 29 is never counted.
    Julian1(u16),
    /// `n`: 0…365, February 29 is counted in leap years.
    Julian0(u16),
}

impl Tz {
    pub const UTC: Tz = Tz { std_off: 0, dst: None };

    pub const fn fixed(off_secs: i32) -> Tz {
        Tz { std_off: off_secs, dst: None }
    }

    /// Rules for an IANA zone name (e.g. `America/New_York`), from the table
    /// generated out of the system tz database.
    pub fn for_tzid(tzid: &str) -> Option<Tz> {
        posix_for(tzid).and_then(Tz::parse)
    }

    pub fn parse(s: &str) -> Option<Tz> {
        let mut p = Parser { b: s.as_bytes(), i: 0 };
        p.name()?;
        let std_off = -p.offset()?;
        if p.done() {
            return Some(Tz { std_off, dst: None });
        }
        p.name()?;
        let dst_off = match p.peek() {
            Some(b'+' | b'-' | b'0'..=b'9') => -p.offset()?,
            _ => std_off + 3600,
        };
        let (start, end) = if p.eat(b',') {
            let start = p.rule()?;
            if !p.eat(b',') {
                return None;
            }
            (start, p.rule()?)
        } else {
            // POSIX leaves the default rule to the implementation; glibc uses
            // the US rules. Real TZif footers always spell the rule out.
            (
                Rule { day: RuleDay::Month { m: 3, w: 2, d: 0 }, time: 7200 },
                Rule { day: RuleDay::Month { m: 11, w: 1, d: 0 }, time: 7200 },
            )
        };
        if !p.done() {
            return None;
        }
        Some(Tz { std_off, dst: Some(Dst { off: dst_off, start, end }) })
    }

    /// Seconds east of UTC in effect at `utc`.
    pub fn offset_at(&self, utc: i64) -> i32 {
        let Some(dst) = &self.dst else {
            return self.std_off;
        };
        let y = LocalTime::from_utc(utc, self.std_off).date.y;
        // The latest transition at or before `utc`, looking one year either
        // side so that rules near New Year (and southern zones) just work.
        let mut best: Option<(i64, i32)> = None;
        for yy in [y - 1, y, y + 1] {
            // Start is given in standard local time, end in daylight time.
            let start = rule_local_secs(dst.start, yy) - self.std_off as i64;
            let end = rule_local_secs(dst.end, yy) - dst.off as i64;
            for (t, off) in [(start, dst.off), (end, self.std_off)] {
                let later = match best {
                    None => true,
                    // On a tie (an "always DST" encoding) prefer DST.
                    Some((bt, _)) => t > bt || (t == bt && off == dst.off),
                };
                if t <= utc && later {
                    best = Some((t, off));
                }
            }
        }
        best.map_or(self.std_off, |(_, off)| off)
    }

    pub fn local(&self, utc: i64) -> LocalTime {
        LocalTime::from_utc(utc, self.offset_at(utc))
    }

    /// UTC instant of local midnight starting `date`.
    pub fn midnight(&self, date: CivilDate) -> i64 {
        // Use the offset in effect a few hours before; transitions never
        // happen right at midnight in practice, and an hour off is harmless
        // for "which day is it" decisions.
        let guess = date.days() * SECS_PER_DAY - self.std_off as i64;
        date.days() * SECS_PER_DAY - self.offset_at(guess) as i64
    }
}

fn rule_local_secs(rule: Rule, year: i32) -> i64 {
    let day = match rule.day {
        RuleDay::Month { m, w, d } => {
            let first = CivilDate::new(year, m, 1);
            let wd1 = first.weekday().index();
            let mut day = 1 + (d + 7 - wd1) % 7 + (w.max(1) - 1) * 7;
            let dim = days_in_month(year, m);
            while day > dim {
                day -= 7;
            }
            CivilDate::new(year, m, day).days()
        }
        RuleDay::Julian1(n) => {
            let mut doy = n.max(1) as i64 - 1;
            if is_leap(year) && n >= 60 {
                doy += 1;
            }
            CivilDate::new(year, 1, 1).days() + doy
        }
        RuleDay::Julian0(n) => CivilDate::new(year, 1, 1).days() + n as i64,
    };
    day * SECS_PER_DAY + rule.time as i64
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn done(&self) -> bool {
        self.i >= self.b.len()
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    /// `<anything>` or at least three letters.
    fn name(&mut self) -> Option<()> {
        if self.eat(b'<') {
            while self.peek()? != b'>' {
                self.i += 1;
            }
            self.i += 1;
            return Some(());
        }
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            self.i += 1;
        }
        (self.i - start >= 3).then_some(())
    }

    fn num(&mut self) -> Option<i32> {
        let start = self.i;
        let mut v: i32 = 0;
        while let Some(c @ b'0'..=b'9') = self.peek() {
            v = v.checked_mul(10)?.checked_add((c - b'0') as i32)?;
            self.i += 1;
        }
        (self.i > start).then_some(v)
    }

    /// `[+|-]h[:mm[:ss]]` in seconds, sign as written.
    fn offset(&mut self) -> Option<i32> {
        let neg = if self.eat(b'-') {
            true
        } else {
            self.eat(b'+');
            false
        };
        let mut secs = self.num()? * 3600;
        if self.eat(b':') {
            secs += self.num()? * 60;
            if self.eat(b':') {
                secs += self.num()?;
            }
        }
        Some(if neg { -secs } else { secs })
    }

    fn rule(&mut self) -> Option<Rule> {
        let day = if self.eat(b'M') {
            let m = self.num()? as u8;
            if !self.eat(b'.') {
                return None;
            }
            let w = self.num()? as u8;
            if !self.eat(b'.') {
                return None;
            }
            let d = self.num()? as u8;
            if !(1..=12).contains(&m) || !(1..=5).contains(&w) || d > 6 {
                return None;
            }
            RuleDay::Month { m, w, d }
        } else if self.eat(b'J') {
            RuleDay::Julian1(self.num()? as u16)
        } else {
            RuleDay::Julian0(self.num()? as u16)
        };
        let time = if self.eat(b'/') { self.offset()? } else { 7200 };
        Some(Rule { day, time })
    }
}

/// POSIX rule string for an IANA zone name.
pub fn posix_for(tzid: &str) -> Option<&'static str> {
    use crate::tz_table::{POSIX, ZONES};
    ZONES.binary_search_by(|(name, _)| (*name).cmp(tzid)).ok().map(|i| POSIX[ZONES[i].1 as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(y: i32, m: u8, d: u8, h: i64, mi: i64) -> i64 {
        CivilDate::new(y, m, d).days() * SECS_PER_DAY + h * 3600 + mi * 60
    }

    #[test]
    fn new_york() {
        let tz = Tz::parse("EST5EDT,M3.2.0,M11.1.0").unwrap();
        // DST 2026 starts Sun Mar 8 02:00 EST = 07:00 UTC.
        assert_eq!(tz.offset_at(utc(2026, 3, 8, 6, 59)), -5 * 3600);
        assert_eq!(tz.offset_at(utc(2026, 3, 8, 7, 0)), -4 * 3600);
        // Ends Sun Nov 1 02:00 EDT = 06:00 UTC.
        assert_eq!(tz.offset_at(utc(2026, 11, 1, 5, 59)), -4 * 3600);
        assert_eq!(tz.offset_at(utc(2026, 11, 1, 6, 0)), -5 * 3600);
    }

    #[test]
    fn jerusalem_thursday_plus_26h() {
        let tz = Tz::parse("IST-2IDT,M3.4.4/26,M10.5.0").unwrap();
        // 2026: 4th Thursday of March is Mar 26, +26 h = Fri Mar 27 02:00 IST = 00:00 UTC.
        assert_eq!(tz.offset_at(utc(2026, 3, 26, 23, 59)), 2 * 3600);
        assert_eq!(tz.offset_at(utc(2026, 3, 27, 0, 0)), 3 * 3600);
        // Last Sunday of October 2026 is Oct 25, 02:00 IDT = Oct 24 23:00 UTC.
        assert_eq!(tz.offset_at(utc(2026, 10, 24, 22, 59)), 3 * 3600);
        assert_eq!(tz.offset_at(utc(2026, 10, 24, 23, 0)), 2 * 3600);
    }

    #[test]
    fn southern_hemisphere_and_quoted_names() {
        let mel = Tz::parse("AEST-10AEDT,M10.1.0,M4.1.0/3").unwrap();
        assert_eq!(mel.offset_at(utc(2027, 1, 15, 0, 0)), 11 * 3600);
        assert_eq!(mel.offset_at(utc(2027, 6, 15, 0, 0)), 10 * 3600);
        let sp = Tz::parse("<-03>3").unwrap();
        assert_eq!(sp.offset_at(utc(2027, 1, 15, 0, 0)), -3 * 3600);
        let nuuk = Tz::parse("<-02>2<-01>,M3.5.0/-1,M10.5.0/0").unwrap();
        assert_eq!(nuuk.offset_at(utc(2027, 7, 1, 0, 0)), -3600);
        assert_eq!(nuuk.offset_at(utc(2027, 1, 1, 0, 0)), -2 * 3600);
        let kolkata = Tz::parse("IST-5:30").unwrap();
        assert_eq!(kolkata.offset_at(0), 5 * 3600 + 1800);
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(Tz::parse(""), None);
        assert_eq!(Tz::parse("EST"), None);
        assert_eq!(Tz::parse("EST5EDT,M13.1.0,M11.1.0"), None);
    }
}
