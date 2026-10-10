//! Every screen renders for every state the recorded calendars produce, and
//! every Hebcal title fits the slots it is shown in.

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use jewcal_model::calendar::CalendarModel;
use jewcal_model::snapshot::Snapshot;
use jewcal_model::time::{CivilDate, SECS_PER_DAY};
use jewcal_model::tz::Tz;
use jewcal_model::wire::{parse_events, Events, Policy};
use jewcal_ui::fonts::{BODY, TINY};
use jewcal_ui::text::{fit, title, wrap};
use jewcal_ui::{available, draw, Canvas, Ctx, Screen, Status};

fn fixture(city: &str, year: i32) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../model/tests/fixtures/events-{city}-{year}.json"))
}

fn model(city: &str, years: &[i32]) -> CalendarModel {
    let mut events = Events::default();
    for &y in years {
        let ev = parse_events(BufReader::new(File::open(fixture(city, y)).unwrap()), Policy::default()).unwrap();
        events.location = ev.location;
        events.items.extend(ev.items);
    }
    let window = (CivilDate::new(years[0], 1, 1), CivilDate::new(years[years.len() - 1], 12, 31));
    CalendarModel::build(events, Vec::new(), window, false, 0)
}

#[test]
fn every_title_fits_without_truncation() {
    use jewcal_model::classify::Kind;
    for city in ["nyc", "jerusalem"] {
        let m = model(city, &[2024, 2025, 2026, 2027, 2028]);
        let uniq = |mut v: Vec<String>| {
            v.sort();
            v.dedup();
            v
        };
        // Agenda rows and the main-screen footer show span titles.
        for t in uniq(m.spans.iter().map(|s| title(&s.title)).collect()) {
            assert!(!fit(&BODY, 1, &t, 104).truncated, "{city}: agenda row {t:?}");
        }
        // The hero shows day titles, parsha and fast names on two 82 px lines.
        let hero = uniq(
            m.items
                .iter()
                .filter(|i| matches!(i.kind, Kind::YomTov | Kind::CholHaMoed | Kind::Fast | Kind::Parsha))
                .map(|i| title(&i.title))
                .collect(),
        );
        for t in &hero {
            let lines = wrap(&BODY, 1, t, 82, 2);
            assert!(lines.iter().all(|l| !fit(&BODY, 1, l, 82).truncated), "{city}: hero {t:?} -> {lines:?}");
        }
        // The period screen shows the parsha alone on one 116 px line.
        for t in hero.iter().filter(|t| !t.contains(' ') || t.contains('-')) {
            assert!(!fit(&BODY, 1, t, 116).truncated, "{city}: parsha line {t:?}");
        }
        // Notes: special Shabbatot and fasts in TINY across the screen.
        let notes = uniq(
            m.items
                .iter()
                .filter(|i| matches!(i.kind, Kind::SpecialShabbat | Kind::Mevarchim | Kind::Fast))
                .map(|i| title(&i.title))
                .collect(),
        );
        for t in notes {
            assert!(!fit(&TINY, 1, &t, 108).truncated, "{city}: note {t:?}");
        }
    }
}

/// A weather reading that cycles through every condition, both units.
fn sample_weather(t: i64) -> jewcal_model::weather::Weather {
    use jewcal_model::weather::{Condition, Current, Day, Units, Weather};
    let i = (t / 3600) as usize;
    let cond = Condition::ALL[i % Condition::ALL.len()];
    let units = [Units::Metric, Units::MetricMs, Units::Imperial][i % 3];
    let date = jewcal_model::time::CivilDate::from_days(t.div_euclid(SECS_PER_DAY));
    Weather {
        units,
        current: Current {
            temp: (i as i32 % 700) - 300,
            feels: Some(-125),
            humidity: Some(100),
            pressure: Some(10_355),
            wind: Some(1234),
            wind_dir: Some((i * 37 % 360) as i32),
            condition: cond,
            is_day: i.is_multiple_of(2),
        },
        days: (0..3)
            .map(|k| Day {
                date: date.add_days(k),
                condition: Condition::ALL[(i + k as usize) % 12],
                max: Some(-105),
                min: Some(-215),
                precip: Some(100),
            })
            .collect(),
        fetched_at: t,
    }
}

#[test]
fn all_screens_render_through_the_years() {
    for (city, tzid) in [("nyc", "America/New_York"), ("jerusalem", "Asia/Jerusalem")] {
        let m = model(city, &[2025, 2026, 2027]);
        let tz = Tz::for_tzid(tzid).unwrap();
        let status = Status::default();
        let mut c = Canvas::new();
        let start = CivilDate::new(2025, 1, 15).days() * SECS_PER_DAY;
        let end = CivilDate::new(2027, 12, 15).days() * SECS_PER_DAY;
        let mut t = start;
        let mut rendered = 0;
        while t < end {
            let s = Snapshot::at(&m, &tz, t);
            let weather = sample_weather(t);
            for screen in std::iter::once(Screen::Main).chain(available(&s, Some(&weather))).chain([Screen::Info]) {
                draw(screen, &mut c, &s, &Ctx { frame: (t / 600) as u32, status: &status, weather: Some(&weather) });
                rendered += 1;
            }
            t += 7 * 3600 + 13 * 60; // walks through every hour of the day over time
        }
        assert!(rendered > 10_000, "{rendered}");
    }
}
