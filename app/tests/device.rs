//! The SmallTV Ultra's loop driven with recorded Hebcal / Open-Meteo bodies
//! in place of the network.

use std::path::PathBuf;

use jewcal_app::device::{Device, Outcome, Request};
use jewcal_app::Settings;
use jewcal_ui::Canvas;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../model/tests/fixtures").join(name)).unwrap()
}

fn nyc() -> Settings {
    Settings { geonameid: Some(5128581), place_name: "New York, NY".into(), cc: "US".into(), ..Settings::default() }
}

/// 2026-10-08 18:00 New York (EDT).
const NOW: i64 = 1_791_496_800;

/// Feed `body` in small chunks, as the firmware's 512-byte reads would.
fn respond(d: &mut Device, body: &[u8], now: i64) {
    for c in body.chunks(512) {
        d.feed(c);
    }
    d.finish(Outcome::Ok, now);
}

fn load(d: &mut Device) {
    let (kind, url) = d.next_request(NOW, true).unwrap();
    assert_eq!(kind, Request::Events);
    assert!(url.starts_with("http://www.hebcal.com/hebcal?v=1&cfg=json&start="), "{url}");
    assert!(url.ends_with("&geonameid=5128581"), "{url}");
    respond(d, &fixture("events-nyc-2026.json"), NOW);
    assert!(!d.has_calendar(), "only after sunrise/sunset");
    let (kind, url) = d.next_request(NOW, true).unwrap();
    assert_eq!(kind, Request::Zmanim);
    assert!(url.starts_with("http://www.hebcal.com/zmanim?cfg=json&start=2026-10-07&end=2026-10-16"), "{url}");
    respond(d, &fixture("zmanim-nyc-2026-10-08.json"), NOW);
    assert!(d.has_calendar());
}

#[test]
fn boot_refresh_then_weather_then_idle() {
    let mut d = Device::new(nyc(), 0, 7);
    assert_eq!(d.next_request(NOW, false), None, "nothing before the clock is set");
    load(&mut d);
    assert_eq!(d.error(), None);
    // The zone Hebcal reported is learned and must be persisted.
    assert!(d.take_settings_dirty());
    assert_eq!(d.settings().tzid, "America/New_York");
    assert!(!d.take_settings_dirty());

    let (kind, url) = d.next_request(NOW, true).unwrap();
    assert_eq!(kind, Request::Weather);
    assert!(url.starts_with("http://api.open-meteo.com/v1/forecast?latitude=40.71427&longitude=-74.00597"), "{url}");
    respond(&mut d, &fixture("weather-nyc.json"), NOW);
    assert!(d.weather().is_some());
    assert_eq!(d.next_request(NOW + 60, true), None);
    // Weather again after half an hour; the calendar after 03:00 tomorrow.
    assert_eq!(d.next_request(NOW + 31 * 60, true).map(|r| r.0), Some(Request::Weather));
    d.finish(Outcome::Network, NOW + 31 * 60);
    assert_eq!(d.next_request(NOW + 32 * 60, true), None);
    let next_day = NOW + 10 * 3600 + 30 * 60 + 60; // 04:31 local, past any jitter
    assert_eq!(d.next_request(next_day, true).map(|r| r.0), Some(Request::Events));

    let mut c = Canvas::new();
    let level = d.frame(&mut c, NOW, 1000);
    assert_eq!(level, 80, "day brightness at 18:00");
    assert_eq!(d.frame(&mut c, NOW + 6 * 3600, 2000), 15, "night brightness at midnight");
}

/// The pixels currently drawn, to tell whether a frame drew anything.
fn pixels(c: &Canvas) -> Vec<u8> {
    let mut out = Vec::new();
    let mut row = [0u8; 120];
    for y in 0..120 {
        c.row(y, &mut row);
        out.extend_from_slice(&row);
    }
    out
}

#[test]
fn a_failed_refresh_backs_off_without_a_retry_storm() {
    let mut d = Device::new(nyc(), 0, 7);
    load(&mut d);
    let mut c = Canvas::new();
    d.frame(&mut c, NOW, 1000);
    let shown = pixels(&c);

    let later = NOW + 86_400;
    assert_eq!(d.next_request(later, true).map(|r| r.0), Some(Request::Events));
    // No room for two calendars: the old one is gone, its last frame stays.
    assert!(!d.has_calendar());
    d.frame(&mut c, later, 2000);
    assert_eq!(pixels(&c), shown, "the panel keeps the last frame during a refresh");

    d.finish(Outcome::Network, later);
    assert_eq!(d.error(), Some("cannot reach hebcal.com"));
    d.frame(&mut c, later, 3000);
    assert_ne!(pixels(&c), shown, "a failed refresh shows the error");
    // No calendar, yet no request on every loop: the backoff holds.
    for t in [1, 10, 59] {
        assert_eq!(d.next_request(later + t, true), None, "+{t}s");
    }
    assert_eq!(d.next_request(later + 60, true).map(|r| r.0), Some(Request::Events));
    d.finish(Outcome::Status { code: 503, retry_after: None }, later + 60);
    assert_eq!(d.error(), Some("hebcal.com: HTTP 503"));
    assert_eq!(d.next_request(later + 60 + 119, true), None);
    assert_eq!(d.next_request(later + 60 + 120, true).map(|r| r.0), Some(Request::Events));
    d.finish(Outcome::Status { code: 429, retry_after: Some(900) }, later + 180);
    assert_eq!(d.next_request(later + 180 + 899, true), None);
    assert_eq!(d.next_request(later + 180 + 900, true).map(|r| r.0), Some(Request::Events));
    // A truncated body is an error too, not an empty calendar.
    d.feed(&fixture("events-nyc-2026.json")[..5000]);
    d.finish(Outcome::Ok, later + 1080);
    assert_eq!(d.error(), Some("bad answer from hebcal.com"));
    assert!(!d.has_calendar());
    // The backoff doubles after the server's 900 s: 1800 s. Then it recovers.
    assert_eq!(d.next_request(later + 1080 + 1799, true), None);
    let back = later + 1080 + 1800;
    assert_eq!(d.next_request(back, true).map(|r| r.0), Some(Request::Events));
    respond(&mut d, &fixture("events-nyc-2026.json"), back);
    assert_eq!(d.next_request(back, true).map(|r| r.0), Some(Request::Zmanim));
    respond(&mut d, &fixture("zmanim-nyc-2026-10-08.json"), back);
    assert!(d.has_calendar());
    assert_eq!(d.error(), None);
}

#[test]
fn an_unknown_city_says_so() {
    let mut d = Device::new(Settings { geonameid: Some(8521754), ..nyc() }, 0, 7);
    assert_eq!(d.next_request(NOW, true).map(|r| r.0), Some(Request::Events));
    d.finish(Outcome::Status { code: 404, retry_after: None }, NOW);
    assert_eq!(d.error(), Some("hebcal.com does not know this city"));
}

#[test]
fn coordinates_send_the_zone_hebcal_reported() {
    let s = Settings { lat: "40.71427".into(), lon: "-74.00597".into(), ..Settings::default() };
    let mut d = Device::new(s, 0, 7);
    let (_, url) = d.next_request(NOW, true).unwrap();
    assert!(url.contains("&latitude=40.71427&longitude=-74.00597") && !url.contains("tzid"), "{url}");
    respond(&mut d, &fixture("events-nyc-2026.json"), NOW);
    let (kind, url) = d.next_request(NOW, true).unwrap();
    assert_eq!(kind, Request::Zmanim);
    assert!(url.ends_with("&tzid=America%2FNew_York"), "{url}");
}

#[test]
fn brightness_before_the_clock_is_set() {
    let s = Settings { bright_day: 70, bright_night: 0, ..nyc() };
    let mut d = Device::new(s, 0, 7);
    let mut c = Canvas::new();
    // A few seconds after 1970 is "midnight", but the clock is not set yet.
    assert_eq!(d.frame(&mut c, 5, 0), 70);
}

#[test]
fn a_new_city_is_fetched_at_once() {
    let mut d = Device::new(nyc(), 0, 7);
    load(&mut d);
    let form = |k: &str| -> Option<String> {
        Some(
            match k {
                "geo" => "281184",
                "name" => "Jerusalem, Israel",
                "cc" => "IL",
                "candle" | "havdalah" | "units" => "0",
                "cycle" => "3",
                "dwell" => "10",
                "bday" => "80",
                "bnight" => "15",
                "nfrom" => "23",
                "nto" => "6",
                _ => return None,
            }
            .to_string(),
        )
    };
    let applied = d.apply_form(form).unwrap();
    assert!(applied.refetch);
    assert!(!d.has_calendar(), "the New York calendar no longer matches the settings");
    let (kind, url) = d.next_request(NOW + 5, true).unwrap();
    assert_eq!(kind, Request::Events);
    assert!(url.ends_with("&geonameid=281184"), "{url}");
    let mut page = String::new();
    d.settings_page(&mut page, "", "<p id=extra>").unwrap();
    assert!(page.contains("loading calendar"));
    assert!(page.contains("<p id=extra>"));
}

#[test]
fn messages_before_a_calendar() {
    let mut d = Device::new(Settings::default(), 0, 7);
    d.set_ip(Some("192.168.1.23".into()));
    let mut c = Canvas::new();
    assert_eq!(d.next_request(NOW, true), None, "no location: nothing to fetch");
    d.frame(&mut c, NOW, 0);
    assert_eq!(
        Device::city_url(" Tel Aviv ").as_deref(),
        Some("http://geocoding-api.open-meteo.com/v1/search?count=10&language=en&format=json&name=Tel%20Aviv")
    );
    assert_eq!(Device::city_url("x"), None);
}
