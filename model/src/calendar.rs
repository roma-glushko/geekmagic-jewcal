//! The calendar model: Hebcal's flat item list turned into **periods** (runs
//! of Shabbat/Yom Tov days framed by candle lighting and havdalah), **fasts**
//! and agenda **spans**.
//!
//! Periods come from the *day structure*: maximal runs of consecutive days
//! that are Shabbat (by weekday) or Yom Tov (`yomtov: true`). Hebcal's times
//! are then attached by date:
//! * the evening before the first day: `candles` at sunset − N minutes;
//! * between two sacred days: `candles` timed at nightfall (YT→YT,
//!   Shabbat→YT) or before sunset (YT on Friday → Shabbat);
//! * the last day: one `havdalah`.
//!
//! Every time is optional. Near the poles in summer the sun never gets 8.5°
//! below the horizon, so Hebcal sends no havdalah and no nightfall candles.
//! Building from candles/havdalah alone would then merge two periods.

use crate::classify::{normalize, Item, Kind};
use crate::prelude::*;
use crate::time::{CivilDate, Stamp, Weekday, SECS_PER_DAY};
use crate::wire::{Events, SunDay, WireLocation};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EveningKind {
    /// Erev Shabbat/Yom Tov, or Yom Tov on Friday going into Shabbat.
    CandlesBeforeSunset,
    /// Second night of Yom Tov, or Motzei Shabbat into Yom Tov: light after
    /// nightfall, from an existing flame.
    CandlesAfterNightfall,
    Havdalah,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evening {
    /// Civil date of the evening (the day *before* the sacred day it opens).
    pub date: CivilDate,
    /// `None` when Hebcal sent no time (high latitudes, window edge).
    pub at: Option<Stamp>,
    pub how: EveningKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeriodDay {
    pub date: CivilDate,
    /// Hebcal title of the Yom Tov, e.g. `Pesach I`, `Rosh Hashana 5788`.
    pub yom_tov: Option<String>,
    pub shabbat: bool,
    /// Shabbat Chol HaMoed: the Chol HaMoed title of that day.
    pub chol_hamoed: Option<String>,
    /// `Parashat …` when a weekly portion is read.
    pub parsha: Option<String>,
    /// Special Shabbat names and "Mevarchim Chodesh …".
    pub specials: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Note {
    /// Yom Tov on Friday leads into Shabbat: make an eruv tavshilin on this date.
    EruvTavshilin(CivilDate),
    /// Shabbat ends into Yom Tov: havdalah is said in the Yom Tov kiddush.
    HavdalahInKiddush(CivilDate),
    /// Chanukah candles are lit before the Shabbat candles that evening.
    ChanukahFirst(CivilDate),
    /// A fast starts before this period ends (Tisha B'Av after Shabbat):
    /// havdalah on wine waits until the fast is over.
    FastFollows { name: String, begins: Stamp },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Period {
    pub days: Vec<PeriodDay>,
    /// `days.len() + 1` evenings: the erev, one between each pair of days,
    /// and havdalah after the last.
    pub evenings: Vec<Evening>,
    pub notes: Vec<Note>,
    pub yom_kippur: bool,
    /// The run touches the edge of the fetched window, so days may be missing.
    pub truncated: bool,
}

impl Period {
    pub fn begins(&self) -> Option<Stamp> {
        self.evenings[0].at
    }

    pub fn ends(&self) -> Option<Stamp> {
        self.evenings[self.evenings.len() - 1].at
    }

    pub fn first_date(&self) -> CivilDate {
        self.days[0].date
    }

    pub fn last_date(&self) -> CivilDate {
        self.days[self.days.len() - 1].date
    }

    /// UTC offset (minutes) seen on any of this period's times.
    fn offset_min(&self) -> i64 {
        self.evenings.iter().find_map(|e| e.at).map_or(0, |s| s.off_min as i64)
    }

    /// Effective time of evening `i`: Hebcal's, or a stand-in when missing
    /// (erev noon for the opening, 06:00 next morning for the closing,
    /// 22:00 for an internal evening).
    pub fn evening_utc(&self, i: usize) -> i64 {
        let e = &self.evenings[i];
        if let Some(s) = e.at {
            return s.utc;
        }
        let hour = if i == 0 {
            12
        } else if i == self.evenings.len() - 1 {
            30
        } else {
            22
        };
        e.date.days() * SECS_PER_DAY + hour * 3600 - self.offset_min() * 60
    }

    pub fn contains(&self, t: i64) -> bool {
        self.evening_utc(0) <= t && t < self.evening_utc(self.evenings.len() - 1)
    }

    /// Index into `days` of the sacred day in progress at `t`.
    pub fn day_index_at(&self, t: i64) -> usize {
        let passed = (1..self.evenings.len() - 1).filter(|&i| self.evening_utc(i) <= t).count();
        passed.min(self.days.len() - 1)
    }

    /// Index of the first evening strictly after `t`.
    pub fn next_evening(&self, t: i64) -> Option<usize> {
        (0..self.evenings.len()).find(|&i| self.evening_utc(i) > t)
    }

    pub fn has_yom_tov(&self) -> bool {
        self.days.iter().any(|d| d.yom_tov.is_some())
    }

    pub fn has_shabbat(&self) -> bool {
        self.days.iter().any(|d| d.shabbat)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fast {
    pub name: String,
    /// The fast day (Tisha B'Av begins the evening before).
    pub date: CivilDate,
    /// Missing at high latitudes in summer, when the sun never reaches the
    /// dawn angle.
    pub begins: Option<Stamp>,
    /// Missing on Friday fasts (they run into Shabbat) and Ta'anit Bechorot.
    pub ends: Option<Stamp>,
    pub major: bool,
}

impl Fast {
    /// Ta'anit Bechorot binds only firstborn sons; it is listed but never
    /// takes over the main screen.
    pub fn firstborn_only(&self) -> bool {
        self.name.starts_with("Ta'anit Bechorot")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpanKind {
    YomTov,
    CholHaMoed,
    Fast,
    Major,
    Minor,
    Modern,
    RoshChodesh,
    Chanukah,
}

/// One agenda line: consecutive days of the same occasion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub first: CivilDate,
    pub last: CivilDate,
    pub title: String,
    pub kind: SpanKind,
    /// Begins the evening of `first` (Chanukah's first candle).
    pub from_evening: bool,
}

#[derive(Clone, Debug, Default)]
pub struct CalendarModel {
    /// Sorted by date, all-day items before timed ones.
    pub items: Vec<Item>,
    pub periods: Vec<Period>,
    pub fasts: Vec<Fast>,
    pub spans: Vec<Span>,
    pub sun: Vec<SunDay>,
    pub location: WireLocation,
    pub israel: bool,
    /// Inclusive date range that was fetched.
    pub window: Option<(CivilDate, CivilDate)>,
    /// Unix time of the fetch, for the staleness marker.
    pub fetched_at: i64,
}

impl CalendarModel {
    pub fn build(
        events: Events,
        mut sun: Vec<SunDay>,
        window: (CivilDate, CivilDate),
        force_israel: bool,
        fetched_at: i64,
    ) -> Self {
        let mut items = events.items;
        crate::sort_by_key(&mut items, |i| (i.date, i.at.map(|s| s.utc)));
        crate::sort_by_key(&mut sun, |s| s.date);
        let periods = build_periods(&items, window);
        let fasts = build_fasts(&items);
        let spans = build_spans(&items);
        let israel = force_israel || events.location.cc.as_deref() == Some("IL");
        Self { items, periods, fasts, spans, sun, location: events.location, israel, window: Some(window), fetched_at }
    }

    pub fn items_on(&self, date: CivilDate) -> &[Item] {
        items_on(&self.items, date)
    }

    pub fn sun(&self, date: CivilDate) -> Option<&SunDay> {
        self.sun.binary_search_by_key(&date, |s| s.date).ok().map(|i| &self.sun[i])
    }

    pub fn yom_tov(&self, date: CivilDate) -> Option<&Item> {
        self.items_on(date).iter().find(|i| i.kind == Kind::YomTov)
    }
}

fn items_on(items: &[Item], date: CivilDate) -> &[Item] {
    let a = items.partition_point(|i| i.date < date);
    let b = items.partition_point(|i| i.date <= date);
    &items[a..b]
}

fn find_kind(items: &[Item], date: CivilDate, kind: Kind) -> Option<&Item> {
    items_on(items, date).iter().find(|i| i.kind == kind)
}

/// Shabbat or Yom Tov.
fn is_sacred(items: &[Item], date: CivilDate) -> bool {
    date.weekday() == Weekday::Sat || find_kind(items, date, Kind::YomTov).is_some()
}

fn build_periods(items: &[Item], (start, end): (CivilDate, CivilDate)) -> Vec<Period> {
    let mut periods = Vec::new();
    let mut run: Vec<CivilDate> = Vec::new();
    let mut d = start;
    while d <= end {
        if is_sacred(items, d) {
            run.push(d);
        } else if !run.is_empty() {
            periods.push(make_period(items, core::mem::take(&mut run), start, end));
        }
        d = d.add_days(1);
    }
    if !run.is_empty() {
        periods.push(make_period(items, run, start, end));
    }
    periods
}

/// The time of the `kind` item belonging to the evening of `date`. Near the
/// poles nightfall can fall after midnight (Helsinki, late May: havdalah at
/// 00:15), and Hebcal then dates the item the next day.
fn evening_time(items: &[Item], date: CivilDate, kind: Kind) -> Option<Stamp> {
    find_kind(items, date, kind)
        .and_then(|i| i.at)
        .or_else(|| find_kind(items, date.add_days(1), kind).and_then(|i| i.at).filter(|s| s.hm().0 < 4))
}

fn make_period(items: &[Item], dates: Vec<CivilDate>, start: CivilDate, end: CivilDate) -> Period {
    let candles_on = |d: CivilDate| evening_time(items, d, Kind::Candles);
    let n = dates.len();

    let mut evenings = Vec::with_capacity(n + 1);
    let erev = dates[0].add_days(-1);
    evenings.push(Evening { date: erev, at: candles_on(erev), how: EveningKind::CandlesBeforeSunset });
    for k in 0..n - 1 {
        let d = dates[k];
        // Shabbat candles always go before sunset, even after a Yom Tov day.
        let how = if dates[k + 1].weekday() == Weekday::Sat {
            EveningKind::CandlesBeforeSunset
        } else {
            EveningKind::CandlesAfterNightfall
        };
        evenings.push(Evening { date: d, at: candles_on(d), how });
    }
    let last = dates[n - 1];
    evenings.push(Evening { date: last, at: evening_time(items, last, Kind::Havdalah), how: EveningKind::Havdalah });

    let days: Vec<PeriodDay> = dates.iter().map(|&d| period_day(items, d)).collect();

    let mut notes = Vec::new();
    if days.iter().any(|d| d.yom_tov.is_some() && d.date.weekday() == Weekday::Fri) && days.iter().any(|d| d.shabbat) {
        notes.push(Note::EruvTavshilin(erev));
    }
    for e in &evenings[1..n] {
        if e.date.weekday() == Weekday::Sat {
            notes.push(Note::HavdalahInKiddush(e.date));
        }
    }
    for e in &evenings[..n] {
        let chanukah = items_on(items, e.date).iter().any(|i| matches!(i.kind, Kind::Chanukah(c) if c > 0));
        if chanukah && e.date.weekday() == Weekday::Fri {
            notes.push(Note::ChanukahFirst(e.date));
        }
    }
    if let (Some(end_at), Some(fb)) = (evenings[n].at, find_kind(items, last, Kind::FastBegins)) {
        if let Some(begins) = fb.at.filter(|b| b.utc <= end_at.utc) {
            let name = fb.memo.clone().map_or_else(|| "Fast".into(), |m| fast_base(&m).to_string());
            notes.push(Note::FastFollows { name, begins });
        }
    }
    let yom_kippur = days.iter().any(|d| d.yom_tov.as_deref().is_some_and(|t| t.starts_with("Yom Kippur")));

    Period { days, evenings, notes, yom_kippur, truncated: dates[0] <= start || last >= end }
}

fn period_day(items: &[Item], date: CivilDate) -> PeriodDay {
    let mut day = PeriodDay {
        date,
        yom_tov: None,
        shabbat: date.weekday() == Weekday::Sat,
        chol_hamoed: None,
        parsha: None,
        specials: Vec::new(),
    };
    for i in items_on(items, date) {
        match i.kind {
            Kind::YomTov if day.yom_tov.is_none() => day.yom_tov = Some(i.title.clone()),
            Kind::CholHaMoed => day.chol_hamoed = Some(i.title.clone()),
            Kind::Parsha => day.parsha = Some(i.title.clone()),
            Kind::SpecialShabbat | Kind::Mevarchim => day.specials.push(i.title.clone()),
            _ => {}
        }
    }
    day
}

/// `Erev Tish'a B'Av` / `Tish'a B'Av (observed)` → `Tish'a B'Av`.
fn fast_base(s: &str) -> &str {
    let s = s.strip_prefix("Erev ").unwrap_or(s);
    s.strip_suffix(" (observed)").unwrap_or(s)
}

fn build_fasts(items: &[Item]) -> Vec<Fast> {
    let edge = |kind: Kind, dates: &[CivilDate], base: &str| -> Option<Stamp> {
        let mut on_dates = items.iter().filter(|i| i.kind == kind && dates.contains(&i.date));
        let named =
            on_dates.clone().find(|i| i.memo.as_deref().is_some_and(|m| fast_base(&normalize(m.to_string())) == base));
        named.or_else(|| on_dates.next()).and_then(|i| i.at)
    };
    items
        .iter()
        .filter(|i| i.kind == Kind::Fast)
        .map(|f| {
            let base = fast_base(&f.title);
            Fast {
                name: f.title.clone(),
                date: f.date,
                begins: edge(Kind::FastBegins, &[f.date, f.date.add_days(-1)], base),
                ends: edge(Kind::FastEnds, &[f.date], base),
                major: base == "Tish'a B'Av",
            }
        })
        .collect()
}

const ROMAN: [&str; 8] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII"];

/// Split a trailing Roman numeral or year: `Pesach VII` → (`Pesach`, `VII`),
/// `Rosh Hashana 5788` → (`Rosh Hashana`, `5788`).
fn without_ordinal(title: &str) -> (&str, Option<&str>) {
    match title.rsplit_once(' ') {
        Some((head, tail)) if ROMAN.contains(&tail) || tail.bytes().all(|b| b.is_ascii_digit()) => (head, Some(tail)),
        _ => (title, None),
    }
}

/// (kind, merge key) for agenda items; `None` for everything else.
fn span_key(item: &Item) -> Option<(SpanKind, String)> {
    let t = item.title.as_str();
    Some(match item.kind {
        Kind::YomTov => {
            let key = match without_ordinal(t).0 {
                "Shmini Atzeret" | "Simchat Torah" => "Shmini Atzeret",
                other => other,
            };
            (SpanKind::YomTov, key.to_string())
        }
        Kind::CholHaMoed if t.contains("Hoshana Raba") => (SpanKind::CholHaMoed, "Hoshana Raba".into()),
        Kind::CholHaMoed => {
            let festival = t.split(' ').next().unwrap_or(t);
            (SpanKind::CholHaMoed, format!("Chol HaMoed {festival}"))
        }
        Kind::Chanukah(_) => (SpanKind::Chanukah, "Chanukah".into()),
        Kind::Fast => (SpanKind::Fast, t.to_string()),
        Kind::Major => (SpanKind::Major, t.to_string()),
        Kind::Minor => (SpanKind::Minor, t.to_string()),
        Kind::Modern => (SpanKind::Modern, t.to_string()),
        Kind::RoshChodesh => (SpanKind::RoshChodesh, t.to_string()),
        _ => return None,
    })
}

fn build_spans(items: &[Item]) -> Vec<Span> {
    struct Acc {
        span: Span,
        key: String,
        ords: (Option<String>, Option<String>),
        titles: Vec<String>,
    }
    let mut acc: Vec<Acc> = Vec::new();
    for item in items {
        let Some((kind, key)) = span_key(item) else {
            continue;
        };
        let ord = without_ordinal(&item.title).1.filter(|o| ROMAN.contains(o)).map(String::from);
        if let Some(a) = acc
            .iter_mut()
            .rev()
            .take(8)
            .find(|a| a.span.kind == kind && a.key == key && item.date <= a.span.last.add_days(1))
        {
            a.span.last = a.span.last.max(item.date);
            if ord.is_some() {
                a.ords.1 = ord;
            }
            if !a.titles.contains(&item.title) {
                a.titles.push(item.title.clone());
            }
            continue;
        }
        acc.push(Acc {
            span: Span {
                first: item.date,
                last: item.date,
                title: String::new(),
                kind,
                from_evening: matches!(item.kind, Kind::Chanukah(n) if n > 0),
            },
            key,
            ords: (ord.clone(), ord),
            titles: vec![item.title.clone()],
        });
    }
    acc.into_iter()
        .map(|mut a| {
            a.span.title = match a.span.kind {
                SpanKind::YomTov if a.key == "Shmini Atzeret" => a.titles.join(" & "),
                SpanKind::YomTov if a.key == "Pesach" || a.key == "Sukkot" => match a.ords {
                    (Some(f), Some(l)) if f != l => format!("{} {f}-{l}", a.key),
                    (Some(f), _) => format!("{} {f}", a.key),
                    _ => a.key.clone(),
                },
                // Keeps the year: "Rosh Hashana 5788".
                SpanKind::YomTov if a.key == "Rosh Hashana" => a.titles[0].clone(),
                SpanKind::YomTov | SpanKind::CholHaMoed | SpanKind::Chanukah => a.key.clone(),
                _ => a.titles[0].clone(),
            };
            a.span
        })
        .collect()
}
