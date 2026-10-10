//! Hebcal.com REST URLs. Hebcal does every halachic computation (candle
//! lighting, nightfall, fasts, Israel/Diaspora, parsha); we only ask.
//!
//! Content is CC BY 4.0: credit "Hebcal.com" wherever it is shown.

use crate::prelude::*;
use crate::time::CivilDate;

pub const HOST: &str = "https://www.hebcal.com";

/// Days of history to fetch, so the period we are in is always complete.
pub const BACK_DAYS: i64 = 7;
/// Days ahead: far enough that "next Yom Tov" is known even in winter.
pub const AHEAD_DAYS: i64 = 200;
/// Candle times, parsha etc. are kept this far ahead; beyond it only holidays.
pub const DETAIL_DAYS: i64 = 45;
/// Sunrise/sunset window (refreshed daily).
pub const SUN_DAYS: i64 = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// GeoNames id, as returned by the `/complete` city search.
    Geoname(u32),
    /// Decimal degrees exactly as the user typed them (never parsed into
    /// floats on the device); Hebcal guesses the zone when `tzid` is `None`.
    Coords { lat: String, lon: String, tzid: Option<String> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Havdalah {
    /// Sun 8.5° below the horizon (Hebcal's default).
    Tzeit85,
    /// Fixed minutes after sunset (42, 50, 72 are common).
    Minutes(u8),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    pub place: Place,
    /// `None` = Hebcal's default: 18 min, Israel 20, Jerusalem 40, Haifa 30.
    pub candle_minutes: Option<u8>,
    pub havdalah: Havdalah,
    /// Israeli locations always get the Israel schedule; this forces it abroad.
    pub force_israel: bool,
    pub modern: bool,
    pub minor: bool,
}

/// Date ranges for one refresh, anchored on the local date `today`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub start: CivilDate,
    pub end: CivilDate,
    pub detail_until: CivilDate,
    pub sun_start: CivilDate,
    pub sun_end: CivilDate,
}

impl Window {
    pub fn around(today: CivilDate) -> Self {
        // Reach back to the start of the Hebrew month too: the month grid
        // marks its holidays.
        let month_start = crate::hebrew::HDate::from_civil(today).month_start().add_days(-2);
        Self {
            start: today.add_days(-BACK_DAYS).min(month_start),
            end: today.add_days(AHEAD_DAYS),
            detail_until: today.add_days(DETAIL_DAYS),
            sun_start: today.add_days(-1),
            sun_end: today.add_days(SUN_DAYS),
        }
    }
}

pub fn events_url(q: &Query, start: CivilDate, end: CivilDate) -> String {
    let mut u = format!(
        "{HOST}/hebcal?v=1&cfg=json&start={start}&end={end}\
         &maj=on&nx=on&mf=on&ss=on&s=on&c=on&leyning=off"
    );
    if q.minor {
        u.push_str("&min=on");
    }
    if q.modern {
        u.push_str("&mod=on");
    }
    match q.havdalah {
        Havdalah::Tzeit85 => u.push_str("&M=on"),
        Havdalah::Minutes(m) => u.push_str(&format!("&m={m}")),
    }
    if let Some(b) = q.candle_minutes {
        u.push_str(&format!("&b={b}"));
    }
    if q.force_israel {
        u.push_str("&i=on");
    }
    push_place(&mut u, &q.place);
    u
}

pub fn zmanim_url(place: &Place, start: CivilDate, end: CivilDate) -> String {
    let mut u = format!("{HOST}/zmanim?cfg=json&start={start}&end={end}");
    push_place(&mut u, place);
    u
}

/// City autocomplete for the settings page.
pub fn complete_url(prefix: &str) -> String {
    let mut u = format!("{HOST}/complete?q=");
    push_encoded(&mut u, prefix);
    u
}

fn push_place(u: &mut String, place: &Place) {
    match place {
        Place::Geoname(id) => u.push_str(&format!("&geonameid={id}")),
        Place::Coords { lat, lon, tzid } => {
            u.push_str("&latitude=");
            push_encoded(u, lat);
            u.push_str("&longitude=");
            push_encoded(u, lon);
            if let Some(tz) = tzid {
                u.push_str("&tzid=");
                push_encoded(u, tz);
            }
        }
    }
}

/// Percent-encode everything outside RFC 3986 "unreserved".
pub fn push_encoded(out: &mut String, s: &str) {
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        let q = Query {
            place: Place::Geoname(5128581),
            candle_minutes: None,
            havdalah: Havdalah::Tzeit85,
            force_israel: false,
            modern: true,
            minor: true,
        };
        let w = Window::around(CivilDate::new(2026, 10, 9));
        assert_eq!(
            events_url(&q, w.start, w.end),
            "https://www.hebcal.com/hebcal?v=1&cfg=json&start=2026-09-10&end=2027-04-27\
             &maj=on&nx=on&mf=on&ss=on&s=on&c=on&leyning=off&min=on&mod=on&M=on&geonameid=5128581"
        );
        let c = Place::Coords { lat: "50.45".into(), lon: "30.52".into(), tzid: Some("Europe/Kyiv".into()) };
        assert!(zmanim_url(&c, w.sun_start, w.sun_end).ends_with("&latitude=50.45&longitude=30.52&tzid=Europe%2FKyiv"));
        assert_eq!(complete_url("Tel Aviv"), "https://www.hebcal.com/complete?q=Tel%20Aviv");
    }
}
