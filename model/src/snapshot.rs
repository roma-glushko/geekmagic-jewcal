//! Everything the screens need at one instant. Recomputed every second on the
//! device; it only borrows from the [`CalendarModel`], so it is cheap.

use crate::calendar::{CalendarModel, Evening, EveningKind, Fast, Period, Span, SpanKind};
use crate::classify::Kind;
use crate::hebrew::{HDate, AV, ELUL, IYYAR, SIVAN, TAMUZ, TISHREI};
use crate::prelude::*;
use crate::time::{CivilDate, LocalTime, Stamp, Weekday};
use crate::tz::Tz;

/// Data older than this is flagged on screen.
pub const STALE_AFTER_SECS: i64 = 36 * 3600;
/// A fast starting within this window takes over the main screen.
pub const FAST_SOON_SECS: i64 = 24 * 3600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sky {
    Day,
    Dusk,
    Night,
}

#[derive(Clone, Copy, Debug)]
pub enum State<'a> {
    /// Between candle lighting and havdalah; `day` indexes `period.days`.
    InPeriod {
        period: &'a Period,
        day: usize,
    },
    InFast(&'a Fast),
    Normal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HebrewDay {
    pub date: HDate,
    /// After sunset the Hebrew date has already advanced.
    pub eve: bool,
    /// The civil date whose daytime this Hebrew day covers.
    pub civil: CivilDate,
}

impl HebrewDay {
    pub fn month_name(&self) -> &'static str {
        self.date.month_name()
    }
}

#[derive(Clone, Debug)]
pub struct ShabbatView<'a> {
    pub date: CivilDate,
    pub period: Option<&'a Period>,
    /// `Parashat …`, absent when a holiday reading replaces it.
    pub parsha: Option<&'a str>,
    /// Yom Tov or Chol HaMoed falling on this Shabbat.
    pub holiday: Option<&'a str>,
    pub specials: Vec<&'a str>,
    /// Friday evening's candle lighting.
    pub candles: Option<&'a Evening>,
    /// Saturday night: havdalah, or after-nightfall candles when Yom Tov
    /// follows.
    pub motzei: Option<&'a Evening>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FestDayKind {
    YomTov,
    CholHaMoed,
}

#[derive(Clone, Debug)]
pub struct FestDay<'a> {
    pub date: CivilDate,
    pub kind: FestDayKind,
    pub title: &'a str,
}

#[derive(Clone, Debug)]
pub struct FestivalView<'a> {
    /// "SUKKOT" or "PESACH".
    pub name: &'static str,
    pub days: Vec<FestDay<'a>>,
}

/// A season of the year, by the Hebrew day (so it turns at sunset).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Season {
    /// The month of preparation before Rosh Hashana; `day` is 1..=29.
    Elul { day: u8 },
    /// The Ten Days of Return, 1–10 Tishrei (Rosh Hashana to Yom Kippur).
    TenDays { day: u8 },
    /// Counting the Omer, 16 Nisan – 5 Sivan; `day` is the count for the
    /// current Hebrew day (after sunset: tonight's count).
    Omer { day: u8 },
    /// The Three Weeks of mourning, 17 Tammuz – 9 Av (`day` 1..=22); the
    /// last nine (1–9 Av, days 14..=22) are the Nine Days.
    ThreeWeeks { day: u8 },
}

impl Season {
    pub fn of(h: HDate) -> Option<Season> {
        if let Some(day) = h.omer() {
            Some(Season::Omer { day })
        } else if h.month == ELUL {
            Some(Season::Elul { day: h.day })
        } else if h.month == TISHREI && h.day <= 10 {
            Some(Season::TenDays { day: h.day })
        } else if h.month == TAMUZ && h.day >= 17 {
            Some(Season::ThreeWeeks { day: h.day - 16 })
        } else if h.month == AV && h.day <= 9 {
            Some(Season::ThreeWeeks { day: 13 + h.day })
        } else {
            None
        }
    }
}

/// Day of the Three Weeks on which the Nine Days begin (1 Av).
pub const NINE_DAYS_FROM: u8 = 14;

/// The 7 sefirot of the Omer count: day `n` is "<day's> sheb'<week's>".
pub const SEFIROT: [&str; 7] = ["Chesed", "Gevurah", "Tiferet", "Netzach", "Hod", "Yesod", "Malchut"];

/// `Tiferet sheb'Gevurah` for day 10 of the Omer.
pub fn sefira(day: u8) -> Option<String> {
    let i = day.checked_sub(1)? as usize;
    (i < 49).then(|| format!("{} sheb'{}", SEFIROT[i % 7], SEFIROT[i / 7]))
}

/// Rosh Hashana that ends the Hebrew year `year` (1 Tishrei of `year + 1`).
pub fn rosh_hashana_after(year: i32) -> CivilDate {
    HDate { year: year + 1, month: TISHREI, day: 1 }.to_civil()
}

/// Lag BaOmer (18 Iyar) and Shavuot (6 Sivan) of a Hebrew year.
pub fn lag_baomer(year: i32) -> CivilDate {
    HDate { year, month: IYYAR, day: 18 }.to_civil()
}

pub fn shavuot(year: i32) -> CivilDate {
    HDate { year, month: SIVAN, day: 6 }.to_civil()
}

/// Holidays with a screen of their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Holiday {
    RoshHashana,
    YomKippur,
    Sukkot,
    Chanukah,
    Purim,
    Pesach,
    Shavuot,
    TishaBav,
}

/// A holiday screen appears this many days ahead and stays until the end.
/// Yom Kippur and Sukkot start right after the holiday before them instead,
/// and Tisha B'Av with the Nine Days (1 Av).
pub const HOLIDAY_LEAD_DAYS: i64 = 7;

#[derive(Clone, Debug)]
pub struct HolidayView<'a> {
    pub holiday: Holiday,
    /// First and last day. Chanukah's `first` is the first candle's evening.
    pub first: CivilDate,
    pub last: CivilDate,
    /// The Shabbat/Yom Tov period containing the first day (with its
    /// evening-by-evening candle lighting).
    pub period: Option<&'a Period>,
    /// Tzom Gedaliah after Rosh Hashana, Ta'anit Esther before Purim, the
    /// fast itself for Tisha B'Av.
    pub fast: Option<&'a Fast>,
    /// Sukkot and Pesach: one entry per festival day.
    pub strip: Vec<FestDay<'a>>,
}

impl HolidayView<'_> {
    /// 0-based day of the holiday that `today` is, if it is one.
    pub fn day_of(&self, today: CivilDate) -> Option<usize> {
        (today >= self.first && today <= self.last).then(|| (today.days() - self.first.days()) as usize)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChanukahView {
    pub first_evening: CivilDate,
    pub last_day: CivilDate,
    /// Candles to light tonight; 0 when there is no lighting tonight.
    pub tonight: u8,
    pub at: Option<Stamp>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DayMark {
    None,
    YomTov,
    CholHaMoed,
    Fast,
    RoshChodesh,
    Modern,
    Holiday,
}

#[derive(Clone, Debug)]
pub struct MonthView {
    pub name: &'static str,
    pub year: i32,
    /// Civil date of day 1.
    pub first: CivilDate,
    pub len: u8,
    /// Today's Hebrew day of month.
    pub today: u8,
    pub marks: Vec<DayMark>,
}

#[derive(Clone, Debug)]
pub struct Snapshot<'a> {
    pub model: &'a CalendarModel,
    pub now: i64,
    pub local: LocalTime,
    pub today: CivilDate,
    pub hebrew: HebrewDay,
    pub sky: Sky,
    pub state: State<'a>,
    /// The next period that has not begun yet.
    pub next_period: Option<&'a Period>,
    /// A fast starting within [`FAST_SOON_SECS`].
    pub fast_soon: Option<&'a Fast>,
    /// Agenda spans not yet over, soonest first (at most 6).
    pub upcoming: Vec<&'a Span>,
    pub next_yom_tov: Option<&'a Span>,
    pub next_rosh_chodesh: Option<&'a Span>,
    pub shabbat: Option<ShabbatView<'a>>,
    /// Holidays near enough for their own screen, soonest first.
    pub holidays: Vec<HolidayView<'a>>,
    pub chanukah: Option<ChanukahView>,
    /// Day of the Omer counted on the current Hebrew day.
    pub omer: Option<u8>,
    /// Elul, the Ten Days of Return, or the Omer.
    pub season: Option<Season>,
    pub month: MonthView,
    pub stale: bool,
}

impl<'a> Snapshot<'a> {
    pub fn at(model: &'a CalendarModel, tz: &Tz, now: i64) -> Self {
        let local = tz.local(now);
        let today = local.date;

        let state = if let Some(p) = model.periods.iter().find(|p| p.contains(now)) {
            State::InPeriod { period: p, day: p.day_index_at(now) }
        } else if let Some(f) = model.fasts.iter().find(|f| {
            let (b, e) = fast_bounds(f, tz);
            !f.firstborn_only() && b <= now && now < e
        }) {
            State::InFast(f)
        } else {
            State::Normal
        };

        let next_period = model.periods.iter().find(|p| p.evening_utc(0) > now);

        let fast_soon = model.fasts.iter().find(|f| {
            let (b, _) = fast_bounds(f, tz);
            !f.firstborn_only() && b > now && b - now <= FAST_SOON_SECS
        });

        let live: Vec<&Span> = model.spans.iter().filter(|s| s.last >= today).collect();
        let next_yom_tov = live.iter().copied().find(|s| s.kind == SpanKind::YomTov && s.first > today);
        let next_rosh_chodesh = live.iter().copied().find(|s| s.kind == SpanKind::RoshChodesh);
        let upcoming = live.into_iter().take(6).collect();

        let hebrew = hebrew_day(model, today, now);
        Self {
            model,
            now,
            local,
            today,
            sky: sky(model, &local, now),
            state,
            next_period,
            fast_soon,
            upcoming,
            next_yom_tov,
            next_rosh_chodesh,
            shabbat: shabbat_view(model, today, now),
            // Holidays begin at sunset: their screens follow the Hebrew day.
            holidays: holiday_views(model, hebrew.civil),
            chanukah: chanukah_view(model, today),
            omer: hebrew.date.omer(),
            season: Season::of(hebrew.date),
            month: month_view(model, &hebrew),
            hebrew,
            stale: now - model.fetched_at > STALE_AFTER_SECS,
        }
    }

    /// The period the main and period screens are about: the current one,
    /// else the next.
    pub fn focus_period(&self) -> Option<&'a Period> {
        match self.state {
            State::InPeriod { period, .. } => Some(period),
            _ => self.next_period,
        }
    }

    pub fn holiday(&self, h: Holiday) -> Option<&HolidayView<'a>> {
        self.holidays.iter().find(|v| v.holiday == h)
    }
}

/// When a fast is in effect. Missing edges fall back to the fast day's local
/// midnight boundaries (a period after a Friday fast takes over at candle
/// lighting anyway).
pub fn fast_bounds(f: &Fast, tz: &Tz) -> (i64, i64) {
    let begins = f.begins.map_or_else(|| tz.midnight(f.date), |s| s.utc);
    let ends = f.ends.map_or_else(|| tz.midnight(f.date.add_days(1)), |s| s.utc);
    (begins, ends)
}

fn hebrew_day(model: &CalendarModel, today: CivilDate, now: i64) -> HebrewDay {
    let eve = model.sun(today).and_then(|s| s.sunset).is_some_and(|s| now >= s.utc);
    let civil = if eve { today.add_days(1) } else { today };
    HebrewDay { date: HDate::from_civil(civil), eve, civil }
}

fn sky(model: &CalendarModel, local: &LocalTime, now: i64) -> Sky {
    const HALF: i64 = 30 * 60;
    let sun = model.sun(local.date);
    match (sun.and_then(|s| s.sunrise), sun.and_then(|s| s.sunset)) {
        (Some(r), Some(s)) => {
            let (r, s) = (r.utc, s.utc);
            if now < r - HALF || now >= s + HALF {
                Sky::Night
            } else if now < r + HALF || now >= s - HALF {
                Sky::Dusk
            } else {
                Sky::Day
            }
        }
        _ => match local.h {
            7..=17 => Sky::Day,
            6 | 18 => Sky::Dusk,
            _ => Sky::Night,
        },
    }
}

fn shabbat_view<'a>(model: &'a CalendarModel, today: CivilDate, now: i64) -> Option<ShabbatView<'a>> {
    let days_to_sat = (Weekday::Sat.index() + 7 - today.weekday().index()) % 7;
    let mut sat = today.add_days(days_to_sat as i64);
    let period_of = |d: CivilDate| model.periods.iter().find(|p| p.days.iter().any(|x| x.date == d));
    // Saturday night: this Shabbat is over, show next week's.
    if let Some(p) = period_of(sat) {
        if let Some(i) = p.evenings.iter().position(|e| e.date == sat) {
            if now >= p.evening_utc(i) {
                sat = sat.add_days(7);
            }
        }
    }
    let period = period_of(sat);
    let items = model.items_on(sat);
    let title_of = |k: Kind| items.iter().find(|i| i.kind == k).map(|i| i.title.as_str());
    Some(ShabbatView {
        date: sat,
        period,
        parsha: title_of(Kind::Parsha),
        holiday: title_of(Kind::YomTov).or_else(|| title_of(Kind::CholHaMoed)),
        specials: items
            .iter()
            .filter(|i| matches!(i.kind, Kind::SpecialShabbat | Kind::Mevarchim))
            .map(|i| i.title.as_str())
            .collect(),
        candles: period
            .and_then(|p| p.evenings.iter().find(|e| e.date == sat.add_days(-1) && e.how != EveningKind::Havdalah)),
        motzei: period.and_then(|p| p.evenings.iter().find(|e| e.date == sat)),
    })
}

fn festival_family(title: &str) -> Option<&'static str> {
    if title.starts_with("Sukkot") || title.starts_with("Shmini Atzeret") || title.starts_with("Simchat Torah") {
        Some("SUKKOT")
    } else if title.starts_with("Pesach") {
        Some("PESACH")
    } else {
        None
    }
}

/// Every Sukkot (through Simchat Torah) and Pesach in the model, as day strips.
fn festival_runs(model: &CalendarModel) -> Vec<FestivalView<'_>> {
    let mut runs: Vec<FestivalView<'_>> = Vec::new();
    for i in model.items.iter().filter(|i| matches!(i.kind, Kind::YomTov | Kind::CholHaMoed)) {
        let Some(name) = festival_family(&i.title) else {
            continue;
        };
        let kind = if i.kind == Kind::YomTov { FestDayKind::YomTov } else { FestDayKind::CholHaMoed };
        let day = FestDay { date: i.date, kind, title: &i.title };
        match runs.last_mut() {
            Some(r) if r.name == name && r.days.last().is_some_and(|d| i.date <= d.date.add_days(1)) => {
                // Two Yom Tov items on one date (Shmini Atzeret + Simchat Torah in Israel).
                if r.days.last().is_some_and(|d| d.date == i.date) {
                    continue;
                }
                r.days.push(day);
            }
            _ => runs.push(FestivalView { name, days: vec![day] }),
        }
    }
    runs
}

/// Holidays whose screen is due today: from [`HOLIDAY_LEAD_DAYS`] before
/// until their last day; Yom Kippur from the day after Rosh Hashana and
/// Sukkot from the day after Yom Kippur, so the screens hand over.
fn holiday_views(model: &CalendarModel, today: CivilDate) -> Vec<HolidayView<'_>> {
    let lead = |d: CivilDate| d.add_days(-HOLIDAY_LEAD_DAYS);
    let yom_tov = |prefix: &'static str| {
        model.spans.iter().filter(move |s| s.kind == SpanKind::YomTov && s.title.starts_with(prefix))
    };
    let fast_named = |prefix: &str, from: CivilDate, to: CivilDate| {
        model.fasts.iter().find(|f| f.name.starts_with(prefix) && f.date >= from && f.date <= to)
    };
    let mut out: Vec<HolidayView<'_>> = Vec::new();
    let mut push = |holiday, first: CivilDate, last: CivilDate, start: CivilDate, fast, strip| {
        if today >= start && today <= last {
            let period = model.periods.iter().find(|p| p.days.iter().any(|d| d.date == first));
            out.push(HolidayView { holiday, first, last, period, fast, strip });
        }
    };

    for rh in yom_tov("Rosh Hashana") {
        let gedaliah = fast_named("Tzom Gedaliah", rh.last.add_days(1), rh.last.add_days(3));
        push(Holiday::RoshHashana, rh.first, rh.last, lead(rh.first), gedaliah, Vec::new());
    }
    for yk in yom_tov("Yom Kippur") {
        let start = yom_tov("Rosh Hashana")
            .find(|rh| rh.last < yk.first && yk.first.days() - rh.last.days() <= 10)
            .map_or(lead(yk.first), |rh| rh.last.add_days(1));
        push(Holiday::YomKippur, yk.first, yk.last, start, None, Vec::new());
    }
    for run in festival_runs(model) {
        let first = run.days[0].date;
        let last = run.days[run.days.len() - 1].date;
        if run.name == "SUKKOT" {
            let start = yom_tov("Yom Kippur")
                .find(|yk| yk.first < first && first.days() - yk.first.days() <= 6)
                .map_or(lead(first), |yk| yk.last.add_days(1));
            push(Holiday::Sukkot, first, last, start, None, run.days);
        } else {
            push(Holiday::Pesach, first, last, lead(first), None, run.days);
        }
    }
    for c in model.spans.iter().filter(|s| s.kind == SpanKind::Chanukah) {
        push(Holiday::Chanukah, c.first, c.last, lead(c.first), None, Vec::new());
    }
    for p in model.spans.iter().filter(|s| s.title == "Purim") {
        let last = model
            .spans
            .iter()
            .find(|s| s.title == "Shushan Purim" && s.first == p.first.add_days(1))
            .map_or(p.last, |s| s.last);
        // 13 Adar, or Thursday 11 Adar when 13 Adar is Shabbat.
        let esther = fast_named("Ta'anit Esther", p.first.add_days(-4), p.first.add_days(-1));
        push(Holiday::Purim, p.first, last, lead(p.first), esther, Vec::new());
    }
    for s in yom_tov("Shavuot") {
        push(Holiday::Shavuot, s.first, s.last, lead(s.first), None, Vec::new());
    }
    // Tisha B'Av (or its observed day, 10 Av, when 9 Av is Shabbat), from
    // the start of the Nine Days.
    for f in model.fasts.iter().filter(|f| f.major) {
        let year = HDate::from_civil(f.date).year;
        let av1 = HDate { year, month: AV, day: 1 }.to_civil();
        push(Holiday::TishaBav, f.date, f.date, av1, Some(f), Vec::new());
    }
    out.sort_by_key(|h| h.first);
    out
}

fn chanukah_view(model: &CalendarModel, today: CivilDate) -> Option<ChanukahView> {
    let span = model
        .spans
        .iter()
        .find(|s| s.kind == SpanKind::Chanukah && today >= s.first.add_days(-HOLIDAY_LEAD_DAYS) && today <= s.last)?;
    let tonight = model.items_on(today).iter().find_map(|i| match i.kind {
        Kind::Chanukah(n) if n > 0 => Some((n, i.at)),
        _ => None,
    });
    Some(ChanukahView {
        first_evening: span.first,
        last_day: span.last,
        tonight: tonight.map_or(0, |t| t.0),
        at: tonight.and_then(|t| t.1),
    })
}

fn month_view(model: &CalendarModel, h: &HebrewDay) -> MonthView {
    let first = h.date.month_start();
    let len = h.date.days_in_month();
    let marks = (0..len as i64)
        .map(|i| {
            let items = model.items_on(first.add_days(i));
            let has = |f: &dyn Fn(Kind) -> bool| items.iter().any(|it| f(it.kind));
            if has(&|k| k == Kind::YomTov) {
                DayMark::YomTov
            } else if has(&|k| k == Kind::Fast) {
                DayMark::Fast
            } else if has(&|k| k == Kind::CholHaMoed) {
                DayMark::CholHaMoed
            } else if has(&|k| k == Kind::RoshChodesh) {
                DayMark::RoshChodesh
            } else if has(&|k| k == Kind::Modern) {
                DayMark::Modern
            } else if has(&|k| matches!(k, Kind::Major | Kind::Minor | Kind::Chanukah(_))) {
                DayMark::Holiday
            } else {
                DayMark::None
            }
        })
        .collect();
    MonthView { name: h.month_name(), year: h.date.year, first, len, today: h.date.day, marks }
}
