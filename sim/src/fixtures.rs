//! Recorded Hebcal responses: download them (`record`), and rebuild the
//! model the device would have built at any instant (`load`).
//!
//! Events are stored per calendar year; at load time they are cut to the
//! same window the device requests (`Window::around(today)`).

use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;
use std::time::Duration;

use jewcal_model::calendar::{CalendarModel, EveningKind, Note};
use jewcal_model::hebcal::{self, Havdalah, Place, Query, Window};
use jewcal_model::snapshot::{fast_bounds, Snapshot, State};
use jewcal_model::time::{CivilDate, Stamp, SECS_PER_DAY};
use jewcal_model::tz::Tz;
use jewcal_model::weather::{self, Units, Weather};
use jewcal_model::wire::{self, Events, Policy};

use crate::cases::{self, Case, City};

pub const DIR: &str = "model/tests/fixtures";
const USER_AGENT: &str = "jewcal-sim/0.1 (fixture recorder for a desk calendar display)";

type Err = Box<dyn std::error::Error>;

pub fn query(city: City) -> Query {
    Query {
        place: Place::Geoname(city.geonameid()),
        candle_minutes: None,
        havdalah: Havdalah::Tzeit85,
        force_israel: false,
        modern: true,
        minor: true,
    }
}

fn events_path(city: City, year: i32) -> PathBuf {
    PathBuf::from(DIR).join(format!("events-{}-{year}.json", city.slug()))
}

fn weather_path(city: City) -> PathBuf {
    PathBuf::from(DIR).join(format!("weather-{}.json", city.slug()))
}

/// One of each unit system across the fixtures.
pub fn units_for(city: City) -> Units {
    match city {
        City::Nyc => Units::Imperial,
        City::Moscow => Units::MetricMs,
        _ => Units::Metric,
    }
}

/// The recorded forecast for the case's city, re-dated to start on the
/// case's day (the gallery shows layout, not that day's real weather).
pub fn weather(case: &Case, now: i64) -> Option<Weather> {
    let f = File::open(weather_path(case.city)).ok()?;
    let mut w = weather::parse(BufReader::new(f), units_for(case.city), now).ok()??;
    let (date, _, _) = parse_local(&case.at).ok()?;
    for (i, d) in w.days.iter_mut().enumerate() {
        d.date = date.add_days(i as i64);
    }
    Some(w)
}

fn zmanim_path(city: City, start: CivilDate) -> PathBuf {
    PathBuf::from(DIR).join(format!("zmanim-{}-{start}.json", city.slug()))
}

fn get(url: &str) -> Result<String, Err> {
    eprintln!("GET {url}");
    let body = ureq::get(url).set("User-Agent", USER_AGENT).timeout(Duration::from_secs(30)).call()?.into_string()?;
    // Be polite: Hebcal asks for well under 90 requests / 10 s.
    std::thread::sleep(Duration::from_millis(400));
    Ok(body)
}

/// Keep only what the device reads from `/zmanim` (sunrise, sunset, tzid).
fn trim_zmanim(body: &str) -> Result<String, Err> {
    let v: serde_json::Value = serde_json::from_str(body)?;
    let trimmed = serde_json::json!({
        "location": { "tzid": v["location"]["tzid"], "cc": v["location"]["cc"], "title": v["location"]["title"] },
        "times": { "sunrise": v["times"]["sunrise"], "sunset": v["times"]["sunset"] },
    });
    Ok(serde_json::to_string_pretty(&trimmed)?)
}

pub fn record_all() -> super::Res {
    fs::create_dir_all(DIR)?;
    for city in [City::Nyc, City::Jerusalem, City::London, City::Moscow, City::Helsinki] {
        for &year in city.years() {
            let path = events_path(city, year);
            if path.exists() {
                continue;
            }
            let url = hebcal::events_url(&query(city), CivilDate::new(year, 1, 1), CivilDate::new(year, 12, 31));
            fs::write(&path, get(&url)?)?;
        }
    }
    // A current forecast per city, at Hebcal's coordinates for it.
    for city in [City::Nyc, City::Jerusalem, City::Moscow] {
        let path = weather_path(city);
        if path.exists() {
            continue;
        }
        let ev =
            wire::parse_events(BufReader::new(File::open(events_path(city, city.years()[0]))?), Policy::default())?;
        if let (Some(lat), Some(lon)) = (ev.location.latitude, ev.location.longitude) {
            fs::write(&path, get(&weather::url(&lat, &lon, units_for(city)))?)?;
        }
    }
    for case in cases::gallery() {
        let (date, _, _) = parse_local(&case.at)?;
        let w = Window::around(date);
        let zm = zmanim_path(case.city, w.sun_start);
        if !zm.exists() {
            let body = get(&hebcal::zmanim_url(&query(case.city).place, w.sun_start, w.sun_end))?;
            fs::write(&zm, trim_zmanim(&body)?)?;
        }
    }
    println!("fixtures up to date in {DIR}");
    Ok(())
}

/// `YYYY-MM-DDTHH:MM` → (date, hour, minute).
pub fn parse_local(s: &str) -> Result<(CivilDate, i64, i64), Err> {
    let date = CivilDate::parse(s).ok_or("bad date, want YYYY-MM-DDTHH:MM")?;
    let h = s.get(11..13).and_then(|x| x.parse().ok()).ok_or("bad hour")?;
    let m = s.get(14..16).and_then(|x| x.parse().ok()).ok_or("bad minute")?;
    Ok((date, h, m))
}

pub fn local_to_utc(tz: &Tz, date: CivilDate, h: i64, m: i64) -> i64 {
    let local = date.days() * SECS_PER_DAY + h * 3600 + m * 60;
    let guess = local - tz.offset_at(local) as i64;
    local - tz.offset_at(guess) as i64
}

/// The model the device would hold at the case's instant.
pub fn load(case: &Case) -> Result<(CalendarModel, Tz, i64), Err> {
    let tz = Tz::for_tzid(case.city.tzid()).ok_or("zone missing from tz table (run gen-tz)")?;
    let (date, h, m) = parse_local(&case.at)?;
    let now = local_to_utc(&tz, date, h, m);
    let w = Window::around(date);
    let policy = Policy { detail_until: Some(w.detail_until), range: Some((w.start, w.end)) };

    let mut events = Events::default();
    for year in w.start.y..=w.end.y {
        let path = events_path(case.city, year);
        if !path.exists() {
            continue;
        }
        let ev = wire::parse_events(BufReader::new(File::open(&path)?), policy)?;
        if events.location.tzid.is_none() {
            events.location = ev.location;
        }
        events.items.extend(ev.items);
    }
    if events.items.is_empty() {
        return Err(format!("no events fixtures for {} around {date}; run `record`", case.city.slug()).into());
    }
    let sun = match File::open(zmanim_path(case.city, w.sun_start)) {
        Ok(f) => wire::parse_zmanim(BufReader::new(f))?.1,
        Err(_) => Vec::new(),
    };
    Ok((CalendarModel::build(events, sun, (w.start, w.end), false, now), tz, now))
}

fn hm(s: Option<Stamp>) -> String {
    s.map_or_else(
        || "--:--".into(),
        |s| {
            let (h, m) = s.hm();
            format!("{} {h:02}:{m:02}", s.local_date().weekday().short())
        },
    )
}

/// Human-readable dump of a snapshot (for eyeballing against hebcal.com).
pub fn describe(s: &Snapshot) -> String {
    let mut o = String::new();
    let l = &s.local;
    let _ = writeln!(o, "now      {} {:02}:{:02} ({})", l.date, l.h, l.m, l.date.weekday().short());
    let h = &s.hebrew;
    let _ =
        writeln!(o, "hebrew   {} {} {}{}", h.date.day, h.month_name(), h.date.year, if h.eve { " (eve)" } else { "" });
    let _ = writeln!(o, "sky      {:?}{}", s.sky, if s.stale { "  STALE" } else { "" });
    match s.state {
        State::InPeriod { period, day } => {
            let _ = writeln!(o, "state    IN PERIOD, day {} of {}", day + 1, period.days.len());
        }
        State::InFast(f) => {
            let _ = writeln!(o, "state    IN FAST {}", f.name);
        }
        State::Normal => {
            let _ = writeln!(o, "state    normal");
        }
    }
    if let Some(p) = s.focus_period() {
        let _ = writeln!(
            o,
            "period   {} day(s){}{}",
            p.days.len(),
            if p.yom_kippur { " [Yom Kippur]" } else { "" },
            if p.truncated { " [truncated]" } else { "" }
        );
        for d in &p.days {
            let mut what: Vec<&str> = Vec::new();
            if let Some(t) = &d.yom_tov {
                what.push(t);
            }
            if d.shabbat {
                what.push("Shabbat");
            }
            if let Some(t) = &d.chol_hamoed {
                what.push(t);
            }
            if let Some(t) = &d.parsha {
                what.push(t);
            }
            what.extend(d.specials.iter().map(String::as_str));
            let _ = writeln!(o, "  day    {} {}  {}", d.date, d.date.weekday().short(), what.join(" | "));
        }
        for e in &p.evenings {
            let how = match e.how {
                EveningKind::CandlesBeforeSunset => "candles (before sunset)",
                EveningKind::CandlesAfterNightfall => "candles (after nightfall, existing flame)",
                EveningKind::Havdalah => "havdalah",
            };
            let _ = writeln!(o, "  eve    {}  {how}", hm(e.at));
        }
        for n in &p.notes {
            let _ = match n {
                Note::EruvTavshilin(d) => {
                    writeln!(o, "  note   eruv tavshilin on {} {d}", d.weekday().short())
                }
                Note::HavdalahInKiddush(d) => {
                    writeln!(o, "  note   havdalah in kiddush on {} {d}", d.weekday().short())
                }
                Note::ChanukahFirst(d) => {
                    writeln!(o, "  note   light Chanukah before Shabbat candles on {d}")
                }
                Note::FastFollows { name, begins } => {
                    writeln!(o, "  note   {name} begins {}", hm(Some(*begins)))
                }
            };
        }
    }
    if let Some(f) = s.fast_soon {
        let _ = writeln!(o, "fast     {} begins {} ends {}", f.name, hm(f.begins), hm(f.ends));
    }
    if let State::InFast(f) = s.state {
        let (b, e) = fast_bounds(f, &Tz::UTC);
        let _ = writeln!(o, "fast     {} ({}..{}) begins {} ends {}", f.name, b, e, hm(f.begins), hm(f.ends));
    }
    if let Some(sh) = &s.shabbat {
        let _ = writeln!(
            o,
            "shabbat  {}  parsha={:?} holiday={:?} specials={:?} candles={} motzei={}",
            sh.date,
            sh.parsha,
            sh.holiday,
            sh.specials,
            hm(sh.candles.and_then(|e| e.at)),
            hm(sh.motzei.and_then(|e| e.at)),
        );
    }
    for sp in &s.upcoming {
        let _ = writeln!(
            o,
            "upcoming {}..{} {:?} {}{}",
            sp.first,
            sp.last,
            sp.kind,
            sp.title,
            if sp.from_evening { " (from evening)" } else { "" }
        );
    }
    if let Some(y) = s.next_yom_tov {
        let _ = writeln!(o, "next YT  {} {}", y.first, y.title);
    }
    if let Some(r) = s.next_rosh_chodesh {
        let _ = writeln!(o, "next RC  {}..{} {}", r.first, r.last, r.title);
    }
    for h in &s.holidays {
        let days: Vec<String> = h.strip.iter().map(|d| format!("{}:{:?}", d.date.d, d.kind)).collect();
        let fast = h.fast.map_or(String::new(), |f| format!(" fast={} {}", f.name, f.date));
        let _ = writeln!(o, "holiday  {:?} {}..{}{} [{}]", h.holiday, h.first, h.last, fast, days.join(" "));
    }
    if let Some(season) = s.season {
        let _ = writeln!(o, "season   {season:?}");
    }
    if let Some(c) = &s.chanukah {
        let _ = writeln!(o, "chanukah {}..{} tonight={} at {}", c.first_evening, c.last_day, c.tonight, hm(c.at));
    }
    let m = &s.month;
    let _ = writeln!(o, "month    {} {} ({} days from {}), today={}", m.name, m.year, m.len, m.first, m.today);
    o
}
