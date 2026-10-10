//! Tests against real Hebcal responses recorded by `jewcal-sim record`.
//!
//! * Oracles: every Hebcal timestamp's UTC offset must equal our POSIX-TZ
//!   evaluation, and every `hdate` must equal our Hebrew-date arithmetic.
//! * Invariants over 5 years × 2 schedules plus high-latitude cities.
//! * Exact expectations for the stacked-holiday cases the display exists for.

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use jewcal_model::calendar::{CalendarModel, EveningKind, Note, Period};
use jewcal_model::classify::{normalize, Kind};
use jewcal_model::hebrew::HDate;
use jewcal_model::snapshot::{sefira, Holiday, Season, Snapshot, State};
use jewcal_model::time::{parse_hebcal_date, CivilDate, Weekday, SECS_PER_DAY};
use jewcal_model::tz::Tz;
use jewcal_model::wire::{parse_events, Events, Policy};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

const CITIES: [(&str, &[i32]); 5] = [
    ("nyc", &[2024, 2025, 2026, 2027, 2028]),
    ("jerusalem", &[2024, 2025, 2026, 2027, 2028]),
    ("london", &[2027]),
    ("moscow", &[2027]),
    ("helsinki", &[2027]),
];

fn events_file(city: &str, year: i32) -> PathBuf {
    dir().join(format!("events-{city}-{year}.json"))
}

/// One model over all recorded years of a city, as if fetched in one go.
fn model(city: &str) -> CalendarModel {
    let years = CITIES.iter().find(|(c, _)| *c == city).unwrap().1;
    let mut events = Events::default();
    for &y in years {
        let f = File::open(events_file(city, y)).expect("run `cargo run -p jewcal-sim -- record`");
        let ev = parse_events(BufReader::new(f), Policy::default()).unwrap();
        events.location = ev.location;
        events.items.extend(ev.items);
    }
    let window = (CivilDate::new(years[0], 1, 1), CivilDate::new(years[years.len() - 1], 12, 31));
    CalendarModel::build(events, Vec::new(), window, false, 0)
}

fn raw_items(city: &str, year: i32) -> Vec<serde_json::Value> {
    let f = File::open(events_file(city, year)).unwrap();
    let v: serde_json::Value = serde_json::from_reader(BufReader::new(f)).unwrap();
    v["items"].as_array().unwrap().clone()
}

fn hm(p: &Period, i: usize) -> Option<(Weekday, u8, u8)> {
    p.evenings[i].at.map(|s| {
        let (h, m) = s.hm();
        (s.local_date().weekday(), h, m)
    })
}

fn period_with(m: &CalendarModel, day: CivilDate) -> &Period {
    m.periods
        .iter()
        .find(|p| p.days.iter().any(|d| d.date == day))
        .unwrap_or_else(|| panic!("no period contains {day}"))
}

// ── Oracles ──────────────────────────────────────────────────────────────────

#[test]
fn tz_matches_every_hebcal_offset() {
    let mut checked = 0;
    for (city, years) in CITIES {
        for &y in years {
            let f = File::open(events_file(city, y)).unwrap();
            let v: serde_json::Value = serde_json::from_reader(BufReader::new(f)).unwrap();
            let tzid = v["location"]["tzid"].as_str().unwrap();
            let tz = Tz::for_tzid(tzid).unwrap_or_else(|| panic!("{tzid} missing from tz table"));
            for item in v["items"].as_array().unwrap() {
                let Some((_, Some(stamp))) = parse_hebcal_date(item["date"].as_str().unwrap()) else {
                    continue;
                };
                assert_eq!(tz.offset_at(stamp.utc), stamp.off_min as i32 * 60, "{city} {tzid}: {}", item["date"]);
                checked += 1;
            }
        }
    }
    assert!(checked > 1500, "only {checked} timed items");
}

#[test]
fn hebrew_dates_match_hebcal() {
    let mut checked = 0;
    for (city, years) in CITIES {
        for &y in years {
            for item in raw_items(city, y) {
                let (Some(date), Some(hdate)) = (item["date"].as_str(), item["hdate"].as_str()) else {
                    continue;
                };
                // All-day items only: a timed evening item may carry the next Hebrew date.
                if date.len() != 10 {
                    continue;
                }
                let h = HDate::from_civil(CivilDate::parse(date).unwrap());
                let ours = format!("{} {} {}", h.day, h.month_name(), h.year);
                assert_eq!(ours, normalize(hdate.to_string()), "{city} {date}");
                checked += 1;
            }
        }
    }
    assert!(checked > 1000, "only {checked} dated items");
}

// ── Invariants ───────────────────────────────────────────────────────────────

#[test]
fn period_invariants() {
    for (city, _) in CITIES {
        let m = model(city);
        assert!(!m.periods.is_empty());
        for p in &m.periods {
            let ctx = format!("{city} period starting {}", p.first_date());
            assert!(p.days.len() <= 3, "{ctx}: {} days", p.days.len());
            assert_eq!(p.evenings.len(), p.days.len() + 1, "{ctx}");
            assert_eq!(p.evenings[0].date, p.first_date().add_days(-1), "{ctx}");
            for (k, d) in p.days.iter().enumerate() {
                if k > 0 {
                    assert_eq!(d.date, p.days[k - 1].date.add_days(1), "{ctx}: gap");
                }
                assert!(d.shabbat || d.yom_tov.is_some(), "{ctx}: ordinary day inside");
                assert_eq!(p.evenings[k + 1].date, d.date, "{ctx}");
            }
            // Times strictly increase where Hebcal gave them.
            let times: Vec<i64> = p.evenings.iter().filter_map(|e| e.at).map(|s| s.utc).collect();
            assert!(times.windows(2).all(|w| w[0] < w[1]), "{ctx}: times out of order");
            for e in &p.evenings {
                match e.how {
                    EveningKind::CandlesBeforeSunset => {
                        assert_ne!(e.date.weekday(), Weekday::Sat, "{ctx}")
                    }
                    EveningKind::CandlesAfterNightfall => {
                        assert_ne!(e.date.weekday(), Weekday::Fri, "{ctx}")
                    }
                    EveningKind::Havdalah => {}
                }
            }
            if !p.truncated && city != "helsinki" {
                assert!(p.evenings.iter().all(|e| e.at.is_some()), "{ctx}: missing time");
            }
            let fri_yt = p.days.iter().any(|d| d.yom_tov.is_some() && d.date.weekday() == Weekday::Fri);
            let eruv = p.notes.iter().any(|n| matches!(n, Note::EruvTavshilin(_)));
            assert_eq!(eruv, fri_yt && p.has_shabbat(), "{ctx}: eruv tavshilin");
            let sat_into_yt = p.evenings[1..p.days.len()].iter().any(|e| e.date.weekday() == Weekday::Sat);
            let kiddush = p.notes.iter().any(|n| matches!(n, Note::HavdalahInKiddush(_)));
            assert_eq!(kiddush, sat_into_yt, "{ctx}: havdalah in kiddush");
        }
        // Periods never overlap and are in order.
        for w in m.periods.windows(2) {
            assert!(w[0].last_date() < w[1].first_date().add_days(-1), "{city}: periods touch");
        }
    }
}

#[test]
fn every_candles_and_havdalah_item_is_used() {
    for (city, _) in CITIES {
        let m = model(city);
        let (_, end) = m.window.unwrap();
        // A Friday at the very end of the window opens a Shabbat we did not fetch.
        for i in m.items.iter().filter(|i| matches!(i.kind, Kind::Candles | Kind::Havdalah) && i.date < end) {
            // After-midnight nightfall belongs to the previous evening.
            let used = m.periods.iter().any(|p| p.evenings.iter().any(|e| e.at == i.at));
            assert!(used, "{city}: unused {:?} on {}", i.kind, i.date);
        }
    }
}

#[test]
fn shabbat_reading_is_parsha_xor_holiday() {
    for (city, _) in CITIES {
        let m = model(city);
        let (start, end) = m.window.unwrap();
        let mut d = start;
        while d <= end {
            if d.weekday() == Weekday::Sat && d > start.add_days(7) && d < end.add_days(-7) {
                let items = m.items_on(d);
                let parsha = items.iter().any(|i| i.kind == Kind::Parsha);
                let holiday = items.iter().any(|i| matches!(i.kind, Kind::YomTov | Kind::CholHaMoed));
                assert!(parsha != holiday, "{city} {d}: parsha={parsha} holiday={holiday}");
            }
            d = d.add_days(1);
        }
    }
}

#[test]
fn israel_has_single_day_yom_tov_except_rosh_hashana() {
    let m = model("jerusalem");
    assert!(m.israel);
    for p in &m.periods {
        for w in p.days.windows(2) {
            if let (Some(a), Some(_)) = (&w[0].yom_tov, &w[1].yom_tov) {
                assert!(a.starts_with("Rosh Hashana"), "two YT days in Israel: {a} on {}", w[0].date);
            }
        }
    }
}

#[test]
fn fasts_begin_before_they_end() {
    for (city, _) in CITIES {
        for f in &model(city).fasts {
            if let (Some(b), Some(e)) = (f.begins, f.ends) {
                assert!(b.utc < e.utc, "{city} {}", f.name);
            }
        }
    }
}

// ── Exact cases ──────────────────────────────────────────────────────────────

#[test]
fn nyc_pesach_2027_is_one_three_day_period() {
    let m = model("nyc");
    let p = period_with(&m, CivilDate::new(2027, 4, 22));
    let titles: Vec<_> = p.days.iter().map(|d| (d.yom_tov.clone(), d.shabbat)).collect();
    assert_eq!(titles, vec![(Some("Pesach I".into()), false), (Some("Pesach II".into()), false), (None, true),]);
    use EveningKind::*;
    let hows: Vec<_> = p.evenings.iter().map(|e| e.how).collect();
    assert_eq!(hows, vec![CandlesBeforeSunset, CandlesAfterNightfall, CandlesBeforeSunset, Havdalah]);
    assert_eq!(hm(p, 0), Some((Weekday::Wed, 19, 23)));
    assert_eq!(hm(p, 1), Some((Weekday::Thu, 20, 26)));
    assert_eq!(hm(p, 2), Some((Weekday::Fri, 19, 25)));
    assert_eq!(hm(p, 3), Some((Weekday::Sat, 20, 28)));
    assert_eq!(p.notes, vec![Note::EruvTavshilin(CivilDate::new(2027, 4, 21))]);
    assert_eq!(p.days[2].chol_hamoed.as_deref(), Some("Pesach III (CH''M)"));
}

#[test]
fn jerusalem_pesach_2027_splits_yom_tov_and_shabbat() {
    let m = model("jerusalem");
    let yt = period_with(&m, CivilDate::new(2027, 4, 22));
    assert_eq!(yt.days.len(), 1);
    assert_eq!(hm(yt, 0), Some((Weekday::Wed, 18, 31))); // 40 minutes in Jerusalem
    assert_eq!(hm(yt, 1), Some((Weekday::Thu, 19, 51)));
    assert!(yt.notes.is_empty());
    let shabbat = period_with(&m, CivilDate::new(2027, 4, 24));
    assert_eq!(shabbat.days.len(), 1);
    assert_eq!(hm(shabbat, 0), Some((Weekday::Fri, 18, 33)));
}

#[test]
fn rosh_hashana_5788_shabbat_into_yom_tov() {
    let m = model("nyc");
    let p = period_with(&m, CivilDate::new(2027, 10, 2));
    assert_eq!(p.days.len(), 2);
    assert!(p.days[0].shabbat && p.days[0].yom_tov.as_deref() == Some("Rosh Hashana 5788"));
    assert_eq!(hm(p, 0), Some((Weekday::Fri, 18, 20)));
    assert_eq!(p.evenings[1].how, EveningKind::CandlesAfterNightfall);
    assert_eq!(hm(p, 1), Some((Weekday::Sat, 19, 17)));
    assert_eq!(hm(p, 2), Some((Weekday::Sun, 19, 16)));
    assert_eq!(p.notes, vec![Note::HavdalahInKiddush(CivilDate::new(2027, 10, 2))]);
}

#[test]
fn rosh_hashana_5785_thursday_to_shabbat() {
    let m = model("nyc");
    let p = period_with(&m, CivilDate::new(2024, 10, 3));
    assert_eq!(p.days.len(), 3);
    assert_eq!(p.notes, vec![Note::EruvTavshilin(CivilDate::new(2024, 10, 2))]);
    // Yom Kippur on Shabbat a week later.
    let yk = period_with(&m, CivilDate::new(2024, 10, 12));
    assert!(yk.yom_kippur && yk.days.len() == 1 && yk.days[0].shabbat);
}

#[test]
fn tisha_bav_after_shabbat_2025() {
    let m = model("nyc");
    let p = period_with(&m, CivilDate::new(2025, 8, 2));
    assert!(p.notes.iter().any(|n| matches!(n, Note::FastFollows { name, .. } if name == "Tish'a B'Av")));
    let f = m.fasts.iter().find(|f| f.date == CivilDate::new(2025, 8, 3)).unwrap();
    assert!(f.major && f.begins.is_some() && f.ends.is_some());
    assert_eq!(f.begins.unwrap().local_date(), CivilDate::new(2025, 8, 2));
}

#[test]
fn friday_fast_has_no_end_time() {
    let m = model("nyc");
    let f = m.fasts.iter().find(|f| f.date == CivilDate::new(2025, 1, 10)).unwrap();
    assert!(f.name.starts_with("Asara B'Tevet") && f.begins.is_some() && f.ends.is_none());
}

#[test]
fn chanukah_before_shabbat_candles() {
    let m = model("nyc");
    let p = period_with(&m, CivilDate::new(2026, 12, 5));
    assert!(p.notes.contains(&Note::ChanukahFirst(CivilDate::new(2026, 12, 4))));
}

#[test]
fn helsinki_summer_has_no_havdalah_but_periods_stay_apart() {
    let m = model("helsinki");
    let shavuot = period_with(&m, CivilDate::new(2027, 6, 12));
    assert_eq!(shavuot.days.len(), 2);
    assert_eq!(shavuot.ends(), None);
    let next = period_with(&m, CivilDate::new(2027, 6, 19));
    assert_eq!(next.days.len(), 1);
}

#[test]
fn after_midnight_havdalah_belongs_to_shabbat() {
    let m = model("helsinki");
    let p = period_with(&m, CivilDate::new(2027, 5, 22));
    let end = p.ends().expect("havdalah dated the next day");
    assert_eq!(end.local_date(), CivilDate::new(2027, 5, 23));
    assert!(end.hm().0 < 4);
}

#[test]
fn parsha_differs_between_israel_and_diaspora() {
    let nyc = model("nyc");
    let jlm = model("jerusalem");
    let parsha = |m: &CalendarModel, d| m.items_on(d).iter().find(|i| i.kind == Kind::Parsha).map(|i| i.title.clone());
    let d = CivilDate::new(2027, 6, 12);
    assert_eq!(parsha(&jlm, d).as_deref(), Some("Parashat Nasso"));
    assert_eq!(parsha(&nyc, d), None); // Shavuot II
    let d = CivilDate::new(2027, 7, 24);
    assert_eq!(parsha(&jlm, d).as_deref(), Some("Parashat Pinchas"));
    assert_eq!(parsha(&nyc, d).as_deref(), Some("Parashat Pinchas"));
}

#[test]
fn spans_merge_festival_days() {
    let m = model("nyc");
    let titles: Vec<&str> = m
        .spans
        .iter()
        .filter(|s| s.first >= CivilDate::new(2027, 4, 20) && s.first <= CivilDate::new(2027, 4, 30))
        .map(|s| s.title.as_str())
        .collect();
    assert_eq!(titles, vec!["Ta'anit Bechorot", "Pesach I-II", "Chol HaMoed Pesach", "Pesach VII-VIII"]);
    let ch = m.spans.iter().find(|s| s.title == "Chanukah" && s.first.y == 2026).unwrap();
    assert_eq!((ch.first, ch.last, ch.from_evening), (CivilDate::new(2026, 12, 4), CivilDate::new(2026, 12, 12), true));
}

// ── Snapshot over time ───────────────────────────────────────────────────────

/// Step through Pesach week in 10-minute increments: the hero must always
/// point at a period that contains `now` (or begins later), and the next
/// evening is always in the future.
#[test]
fn snapshot_is_consistent_through_pesach_week() {
    let m = model("nyc");
    let tz = Tz::for_tzid("America/New_York").unwrap();
    let start = CivilDate::new(2027, 4, 19).days() * SECS_PER_DAY;
    let mut in_period_minutes = 0;
    for step in 0..(10 * 24 * 6) {
        let now = start + step * 600;
        let s = Snapshot::at(&m, &tz, now);
        match s.state {
            State::InPeriod { period, day } => {
                assert!(period.contains(now));
                assert!(day < period.days.len());
                let next = period.next_evening(now).expect("an evening remains");
                assert!(period.evening_utc(next) > now);
                in_period_minutes += 10;
            }
            State::InFast(f) => assert!(!f.firstborn_only()),
            State::Normal => {}
        }
        if let Some(p) = s.next_period {
            assert!(p.evening_utc(0) > now);
        }
        assert!(s.month.today >= 1 && s.month.today <= s.month.len);
    }
    // Pesach I–II + Shabbat (3 days) and Pesach VII–VIII (2 days), ~25 h each.
    assert!(in_period_minutes > 4 * 24 * 60, "{in_period_minutes}");
}

// ── Holiday screens ──────────────────────────────────────────────────────────

fn holidays_at(m: &CalendarModel, tz: &Tz, y: i32, mo: u8, d: u8, h: i64) -> Vec<Holiday> {
    let t = CivilDate::new(y, mo, d).days() * SECS_PER_DAY + h * 3600
        - tz.offset_at(CivilDate::new(y, mo, d).days() * SECS_PER_DAY) as i64;
    Snapshot::at(m, tz, t).holidays.iter().map(|v| v.holiday).collect()
}

/// A city model with the recorded sunsets of one `zmanim-*` window, so the
/// Hebrew day rolls over at sunset as it does on the device.
fn model_with_sun(city: &str, zmanim: &str) -> CalendarModel {
    let base = model(city);
    let f = File::open(dir().join(format!("zmanim-{city}-{zmanim}.json"))).unwrap();
    let sun = jewcal_model::wire::parse_zmanim(BufReader::new(f)).unwrap().1;
    let events = Events { location: base.location.clone(), items: base.items.clone() };
    CalendarModel::build(events, sun, base.window.unwrap(), false, 0)
}

#[test]
fn holiday_screens_appear_and_hand_over() {
    let m = model_with_sun("nyc", "2027-10-18");
    let tz = Tz::for_tzid("America/New_York").unwrap();
    // A week ahead of Rosh Hashana 5788 (Sat Oct 2, 2027).
    assert_eq!(holidays_at(&m, &tz, 2027, 9, 24, 12), vec![]);
    assert_eq!(holidays_at(&m, &tz, 2027, 9, 25, 12), vec![Holiday::RoshHashana]);
    // The day after Rosh Hashana, Yom Kippur takes over; after it, Sukkot.
    assert_eq!(holidays_at(&m, &tz, 2027, 10, 4, 12), vec![Holiday::YomKippur]);
    assert_eq!(holidays_at(&m, &tz, 2027, 10, 12, 12), vec![Holiday::Sukkot]);
    // Sukkot ends with Simchat Torah (Sun Oct 24); gone after its sunset.
    assert_eq!(holidays_at(&m, &tz, 2027, 10, 24, 12), vec![Holiday::Sukkot]);
    assert_eq!(holidays_at(&m, &tz, 2027, 10, 24, 22), vec![]);
    // Erev Pesach after sunset is already the first day.
    assert_eq!(holidays_at(&m, &tz, 2027, 4, 21, 22), vec![Holiday::Pesach]);
    // Chanukah 5787 (first candle Fri Dec 4, 2026) shows a week ahead.
    assert_eq!(holidays_at(&m, &tz, 2026, 11, 27, 12), vec![Holiday::Chanukah]);
}

#[test]
fn holiday_details() {
    let m = model("nyc");
    let tz = Tz::for_tzid("America/New_York").unwrap();
    let at = |y, mo, d, h: i64| CivilDate::new(y, mo, d).days() * SECS_PER_DAY + (h + 4) * 3600;

    let s = Snapshot::at(&m, &tz, at(2027, 3, 19, 12));
    let purim = s.holiday(Holiday::Purim).expect("purim screen");
    assert_eq!((purim.first, purim.last), (CivilDate::new(2027, 3, 23), CivilDate::new(2027, 3, 24)));
    let esther = purim.fast.expect("Ta'anit Esther");
    assert_eq!(esther.date, CivilDate::new(2027, 3, 22));
    assert!(esther.begins.is_some() && esther.ends.is_some());

    let s = Snapshot::at(&m, &tz, at(2027, 6, 5, 12));
    assert_eq!(s.omer, Some(44));
    let shavuot = s.holiday(Holiday::Shavuot).expect("shavuot screen");
    assert_eq!(shavuot.period.map(|p| p.days.len()), Some(2));

    let s = Snapshot::at(&m, &tz, at(2027, 9, 28, 12));
    let rh = s.holiday(Holiday::RoshHashana).expect("rosh hashana screen");
    assert_eq!(rh.fast.map(|f| f.date), Some(CivilDate::new(2027, 10, 4)));

    let s = Snapshot::at(&m, &tz, at(2027, 10, 19, 12));
    let sukkot = s.holiday(Holiday::Sukkot).expect("sukkot screen");
    assert_eq!(sukkot.strip.len(), 9, "Diaspora: 7 days + Shmini Atzeret + Simchat Torah");
    let jlm = model("jerusalem");
    let tz = Tz::for_tzid("Asia/Jerusalem").unwrap();
    let s = Snapshot::at(&jlm, &tz, at(2027, 10, 19, 9));
    assert_eq!(s.holiday(Holiday::Sukkot).map(|h| h.strip.len()), Some(8), "Israel: 7 days + Shmini Atzeret");
}

#[test]
fn seasons_follow_the_hebrew_day() {
    let m = model_with_sun("nyc", "2027-10-18");
    let tz = Tz::for_tzid("America/New_York").unwrap();
    let noon = |y, mo, d| CivilDate::new(y, mo, d).days() * SECS_PER_DAY + 16 * 3600;
    let season = |t| Snapshot::at(&m, &tz, t).season;
    // 29 Elul 5787 is Fri Oct 1, 2027; Rosh Hashana starts that evening.
    assert_eq!(season(noon(2027, 10, 1)), Some(Season::Elul { day: 29 }));
    assert_eq!(season(noon(2027, 10, 2)), Some(Season::TenDays { day: 1 }));
    assert_eq!(season(noon(2027, 10, 11)), Some(Season::TenDays { day: 10 }));
    assert_eq!(season(noon(2027, 10, 12)), None);
    // Omer: 16 Nisan 5787 = Fri Apr 23, 2027 (day 1); 5 Sivan = Thu Jun 10 (day 49).
    assert_eq!(season(noon(2027, 4, 22)), None);
    assert_eq!(season(noon(2027, 4, 23)), Some(Season::Omer { day: 1 }));
    assert_eq!(season(noon(2027, 5, 25)), Some(Season::Omer { day: 33 }));
    assert_eq!(season(noon(2027, 6, 10)), Some(Season::Omer { day: 49 }));
    assert_eq!(season(noon(2027, 6, 11)), None);
    // Sunset on Simchat Torah (Sun Oct 24) turns the Hebrew day; at 22:00 there is no season.
    let late = CivilDate::new(2027, 10, 24).days() * SECS_PER_DAY + 26 * 3600;
    assert_eq!(season(late), None);
    assert_eq!(sefira(1).as_deref(), Some("Chesed sheb'Chesed"));
    assert_eq!(sefira(10).as_deref(), Some("Tiferet sheb'Gevurah"));
    assert_eq!(sefira(49).as_deref(), Some("Malchut sheb'Malchut"));
    assert_eq!(sefira(50), None);
}
