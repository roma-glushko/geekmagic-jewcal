//! What the user configures, and what it means for Hebcal and the display.
//!
//! Each firmware persists it its own way: NVS keys on the ESP32, the
//! [`Settings::to_text`] form in EEPROM on the ESP8266.

use crate::prelude::*;
use jewcal_model::hebcal::{Havdalah, Place, Query};
use jewcal_model::weather::Units;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// WiFi credentials (the ESP32 keeps them here; the ESP8266 SDK stores
    /// its own, so they stay empty there).
    pub ssid: String,
    pub pass: String,
    /// GeoNames id picked through the city search.
    pub geonameid: Option<u32>,
    /// Or decimal degrees as typed (kept as text: no floats needed).
    pub lat: String,
    pub lon: String,
    pub place_name: String,
    /// Country code of the picked city (`IL` gets the Israel schedule).
    pub cc: String,
    /// IANA zone; learned from Hebcal's response, used for the clock.
    pub tzid: String,
    /// Minutes before sunset; 0 = Hebcal's default (18, Jerusalem 40…).
    pub candle_minutes: u8,
    /// Minutes after sunset; 0 = nightfall at 8.5°.
    pub havdalah_minutes: u8,
    pub force_israel: bool,
    pub modern: bool,
    pub minor: bool,
    /// Secondary screens rotate in every `cycle_min` minutes…
    pub cycle_min: u8,
    /// …each shown for `dwell_s` seconds.
    pub dwell_s: u8,
    pub bright_day: u8,
    pub bright_night: u8,
    pub night_from: u8,
    pub night_to: u8,
    /// Weather units: 0 auto (imperial in the US), 1 °C km/h hPa,
    /// 2 °C m/s mmHg, 3 °F mph inHg.
    pub units: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ssid: String::new(),
            pass: String::new(),
            geonameid: None,
            lat: String::new(),
            lon: String::new(),
            place_name: String::new(),
            cc: String::new(),
            tzid: String::new(),
            candle_minutes: 0,
            havdalah_minutes: 0,
            force_israel: false,
            modern: true,
            minor: true,
            cycle_min: 3,
            dwell_s: 10,
            bright_day: 80,
            bright_night: 15,
            night_from: 23,
            night_to: 6,
            units: 0,
        }
    }
}

impl Settings {
    pub fn place(&self) -> Option<Place> {
        if let Some(id) = self.geonameid {
            return Some(Place::Geoname(id));
        }
        if !self.lat.is_empty() && !self.lon.is_empty() {
            return Some(Place::Coords {
                lat: self.lat.clone(),
                lon: self.lon.clone(),
                tzid: (!self.tzid.is_empty()).then(|| self.tzid.clone()),
            });
        }
        None
    }

    pub fn query(&self) -> Option<Query> {
        Some(Query {
            place: self.place()?,
            candle_minutes: (self.candle_minutes > 0).then_some(self.candle_minutes),
            havdalah: match self.havdalah_minutes {
                0 => Havdalah::Tzeit85,
                m => Havdalah::Minutes(m),
            },
            force_israel: self.force_israel,
            modern: self.modern,
            minor: self.minor,
        })
    }

    /// Everything that changes what Hebcal returns. A calendar fetched under
    /// another key belongs to another place or custom and must not be shown.
    pub fn fetch_key(&self) -> String {
        format!(
            "{:?}|{}|{}|{}|{}|{}|{}|{}",
            self.geonameid,
            self.lat,
            self.lon,
            self.candle_minutes,
            self.havdalah_minutes,
            self.force_israel,
            self.modern,
            self.minor
        )
    }

    pub fn units(&self) -> Units {
        match self.units {
            1 => Units::Metric,
            2 => Units::MetricMs,
            3 => Units::Imperial,
            _ if self.cc == "US" => Units::Imperial,
            _ => Units::Metric,
        }
    }

    pub fn is_night(&self, hour: u8) -> bool {
        let (from, to) = (self.night_from, self.night_to);
        if from == to {
            false
        } else if from < to {
            hour >= from && hour < to
        } else {
            hour >= from || hour < to
        }
    }

    pub fn brightness(&self, hour: u8) -> u8 {
        if self.is_night(hour) {
            self.bright_night
        } else {
            self.bright_day
        }
    }

    /// `key=value` lines (same keys as the ESP32's NVS), without the WiFi
    /// credentials. Newlines inside values become spaces.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let mut put = |k: &str, v: &str| {
            out.push_str(k);
            out.push('=');
            out.extend(v.chars().map(|c| if c == '\n' || c == '\r' { ' ' } else { c }));
            out.push('\n');
        };
        put("geo", &self.geonameid.map_or(String::new(), |g| g.to_string()));
        put("lat", &self.lat);
        put("lon", &self.lon);
        put("name", &self.place_name);
        put("cc", &self.cc);
        put("tzid", &self.tzid);
        for (k, v) in [
            ("candle", self.candle_minutes),
            ("havdalah", self.havdalah_minutes),
            ("israel", self.force_israel as u8),
            ("modern", self.modern as u8),
            ("minor", self.minor as u8),
            ("cycle", self.cycle_min),
            ("dwell", self.dwell_s),
            ("bday", self.bright_day),
            ("bnight", self.bright_night),
            ("nfrom", self.night_from),
            ("nto", self.night_to),
            ("units", self.units),
        ] {
            put(k, &v.to_string());
        }
        out
    }

    /// Inverse of [`to_text`](Self::to_text). Unknown keys and unparsable
    /// numbers keep their defaults, so older or newer layouts still load.
    pub fn from_text(text: &str) -> Self {
        let mut s = Settings::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let n = |d: u8| v.trim().parse::<u8>().unwrap_or(d);
            match k {
                "geo" => s.geonameid = v.trim().parse().ok().filter(|&g| g > 0),
                "lat" => s.lat = v.to_string(),
                "lon" => s.lon = v.to_string(),
                "name" => s.place_name = v.to_string(),
                "cc" => s.cc = v.to_string(),
                "tzid" => s.tzid = v.to_string(),
                "candle" => s.candle_minutes = n(s.candle_minutes),
                "havdalah" => s.havdalah_minutes = n(s.havdalah_minutes),
                "israel" => s.force_israel = n(s.force_israel as u8) == 1,
                "modern" => s.modern = n(s.modern as u8) == 1,
                "minor" => s.minor = n(s.minor as u8) == 1,
                "cycle" => s.cycle_min = n(s.cycle_min),
                "dwell" => s.dwell_s = n(s.dwell_s),
                "bday" => s.bright_day = n(s.bright_day),
                "bnight" => s.bright_night = n(s.bright_night),
                "nfrom" => s.night_from = n(s.night_from),
                "nto" => s.night_to = n(s.night_to),
                "units" => s.units = n(s.units),
                _ => {}
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_roundtrip() {
        let s = Settings {
            geonameid: Some(281184),
            place_name: "Jerusalem, Israel".into(),
            cc: "IL".into(),
            tzid: "Asia/Jerusalem".into(),
            candle_minutes: 40,
            havdalah_minutes: 72,
            force_israel: true,
            modern: false,
            minor: true,
            cycle_min: 5,
            dwell_s: 12,
            bright_day: 90,
            bright_night: 0,
            night_from: 22,
            night_to: 7,
            units: 2,
            ..Settings::default()
        };
        assert_eq!(Settings::from_text(&s.to_text()), s);
        let c = Settings {
            lat: "50.4501".into(),
            lon: "-30.5234".into(),
            place_name: "a\nb".into(),
            ..Settings::default()
        };
        let back = Settings::from_text(&c.to_text());
        assert_eq!((back.lat.as_str(), back.lon.as_str(), back.place_name.as_str()), ("50.4501", "-30.5234", "a b"));
        assert_eq!(back.geonameid, None);
    }

    #[test]
    fn garbage_keeps_defaults() {
        let s = Settings::from_text("cycle=abc\nfuture=1\nno equals sign\ngeo=0\n");
        assert_eq!(s, Settings::default());
        assert_eq!(Settings::from_text(""), Settings::default());
    }

    #[test]
    fn night_window() {
        let s = Settings::default();
        assert!(s.is_night(23) && s.is_night(2) && !s.is_night(6) && !s.is_night(12));
        let day = Settings { night_from: 1, night_to: 5, ..Settings::default() };
        assert!(day.is_night(1) && day.is_night(4) && !day.is_night(5) && !day.is_night(0));
        assert!(!Settings { night_from: 3, night_to: 3, ..Settings::default() }.is_night(3));
    }
}
