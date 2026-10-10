//! Hebcal JSON, deserialized with **no floating-point fields** and streamed.
//!
//! * serde_json's float parser does not codegen on Xtensa ("Cannot select
//!   XtensaISD::PCREL_WRAPPER"). The only floats Hebcal sends are
//!   `location.latitude/longitude`, which are kept as raw text.
//! * The events response is ~40 KB: more than the ESP8266 heap, and a lot for
//!   the ESP32 without PSRAM. [`json::Splitter`](crate::json::Splitter) cuts
//!   the body into items as it streams in; each is classified on its own and
//!   only the ones the display needs are kept.

use crate::prelude::*;
use alloc::collections::BTreeMap;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use crate::classify::{classify, Item};
use crate::json::{Path, Seg, Sink, Splitter};
use crate::time::{parse_hebcal_date, CivilDate, Stamp};

/// Longest single item kept (Hebcal's longest, a holiday with its memo, is
/// under 1 KB).
const ITEM_LIMIT: usize = 4096;

#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq)]
pub struct WireLocation {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub tzid: Option<String>,
    #[serde(default)]
    pub cc: Option<String>,
    #[serde(default)]
    pub geonameid: Option<u32>,
    /// Decimal degrees exactly as Hebcal wrote them: captured as raw JSON
    /// text, never parsed as floats (Xtensa), and passed on to the weather.
    #[serde(default, deserialize_with = "raw_text")]
    pub latitude: Option<String>,
    #[serde(default, deserialize_with = "raw_text")]
    pub longitude: Option<String>,
}

fn raw_text<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let raw: Option<Box<serde_json::value::RawValue>> = Option::deserialize(d)?;
    Ok(raw.map(|r| r.get().to_string()))
}

#[derive(Debug, Default, Deserialize)]
pub struct WireItem {
    #[serde(default)]
    pub title: Option<String>,
    /// Plain-ASCII title when `title` differs (curly quotes, or a time as in
    /// "Candle lighting: 7:23pm").
    #[serde(default)]
    pub title_orig: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub subcat: Option<String>,
    #[serde(default)]
    pub yomtov: Option<bool>,
    #[serde(default)]
    pub memo: Option<String>,
}

/// Which items to keep from the events stream.
#[derive(Clone, Copy, Debug, Default)]
pub struct Policy {
    /// Beyond this date only agenda items (holidays, Rosh Chodesh) are kept.
    pub detail_until: Option<CivilDate>,
    /// Drop anything outside this inclusive range (used when a fixture
    /// covers more than the device would have fetched).
    pub range: Option<(CivilDate, CivilDate)>,
}

impl Policy {
    fn keep(&self, item: &Item) -> bool {
        if let Some((a, b)) = self.range {
            if item.date < a || item.date > b {
                return false;
            }
        }
        match self.detail_until {
            Some(limit) if item.date > limit => item.kind.is_agenda(),
            _ => true,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Events {
    pub location: WireLocation,
    pub items: Vec<Item>,
}

/// Incremental `/hebcal` parser: [`feed`](Self::feed) the body as it
/// arrives, in chunks of any size.
pub struct EventsParser {
    split: Splitter,
    sink: EventsSink,
}

struct EventsSink {
    policy: Policy,
    out: Events,
    error: Option<serde_json::Error>,
    skipped: usize,
}

const LOCATION: u8 = 0;
const ITEM: u8 = 1;

impl Sink for EventsSink {
    fn select(&mut self, path: &Path) -> Option<u8> {
        if path.is(&[Seg::Key("items"), Seg::Elem]) {
            Some(ITEM)
        } else if path.is(&[Seg::Key("location")]) {
            Some(LOCATION)
        } else {
            None
        }
    }

    fn value(&mut self, tag: u8, raw: Option<&[u8]>) {
        if self.error.is_some() {
            return;
        }
        let Some(raw) = raw else {
            self.skipped += 1;
            return;
        };
        let result = match tag {
            LOCATION => {
                serde_json::from_slice::<Option<WireLocation>>(raw).map(|l| self.out.location = l.unwrap_or_default())
            }
            _ => serde_json::from_slice::<WireItem>(raw).map(|w| {
                if let Some(item) = classify(w) {
                    if self.policy.keep(&item) {
                        self.out.items.push(item);
                    }
                }
            }),
        };
        if let Err(e) = result {
            self.error = Some(e);
        }
    }
}

impl EventsParser {
    pub fn new(policy: Policy) -> Self {
        Self {
            split: Splitter::new(ITEM_LIMIT),
            sink: EventsSink { policy, out: Events::default(), error: None, skipped: 0 },
        }
    }

    pub fn feed(&mut self, chunk: &[u8]) {
        if self.sink.error.is_none() {
            self.split.feed(chunk, &mut self.sink);
        }
    }

    /// Items received so far (to watch memory while streaming).
    pub fn items(&self) -> usize {
        self.sink.out.items.len()
    }

    /// Items longer than the limit, skipped.
    pub fn skipped(&self) -> usize {
        self.sink.skipped
    }

    pub fn finish(mut self) -> serde_json::Result<Events> {
        self.take()
    }

    /// [`finish`](Self::finish) in place: a boxed parser is not copied onto
    /// the caller's stack (the ESP8266's is 4 KB).
    pub fn take(&mut self) -> serde_json::Result<Events> {
        let end = self.split.finish(&mut self.sink);
        if let Some(e) = self.sink.error.take() {
            return Err(e);
        }
        end.map_err(serde_json::Error::custom)?;
        Ok(core::mem::take(&mut self.sink.out))
    }
}

/// Stream-parse a `/hebcal` response from a reader.
#[cfg(feature = "std")]
pub fn parse_events<R: std::io::Read>(reader: R, policy: Policy) -> serde_json::Result<Events> {
    let mut p = EventsParser::new(policy);
    read_chunks(reader, |c| p.feed(c))?;
    p.finish()
}

#[cfg(feature = "std")]
fn read_chunks<R: std::io::Read>(mut reader: R, mut f: impl FnMut(&[u8])) -> serde_json::Result<()> {
    let mut buf = [0u8; 512];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(n) => f(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(serde_json::Error::io(e)),
        }
    }
}

/// Sunrise and sunset for one civil date.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SunDay {
    pub date: CivilDate,
    pub sunrise: Option<Stamp>,
    pub sunset: Option<Stamp>,
}

/// Incremental `/zmanim?…&start=…&end=…` parser keeping only sunrise and
/// sunset (the other ~35 zmanim are skipped as they stream past).
pub struct ZmanimParser {
    split: Splitter,
    sink: ZmanimSink,
}

#[derive(Default)]
struct ZmanimSink {
    location: Option<WireLocation>,
    days: BTreeMap<CivilDate, SunDay>,
    error: Option<serde_json::Error>,
}

const SUNRISE: u8 = 1;
const SUNSET: u8 = 2;

impl Sink for ZmanimSink {
    fn select(&mut self, path: &Path) -> Option<u8> {
        if path.is(&[Seg::Key("location")]) {
            Some(LOCATION)
        } else if path.is(&[Seg::Key("times"), Seg::Key("sunrise")]) {
            Some(SUNRISE)
        } else if path.is(&[Seg::Key("times"), Seg::Key("sunset")]) {
            Some(SUNSET)
        } else {
            None
        }
    }

    fn value(&mut self, tag: u8, raw: Option<&[u8]>) {
        if self.error.is_some() {
            return;
        }
        let Some(raw) = raw else {
            self.error = Some(serde_json::Error::custom("zmanim value too long"));
            return;
        };
        // Separate frames: on the ESP8266 this runs deep in a 4 KB stack.
        let result = match tag {
            LOCATION => self.location(raw),
            _ => self.times(tag, raw),
        };
        if let Err(e) = result {
            self.error = Some(e);
        }
    }
}

impl ZmanimSink {
    #[inline(never)]
    fn location(&mut self, raw: &[u8]) -> serde_json::Result<()> {
        self.location = serde_json::from_slice::<Option<WireLocation>>(raw)?;
        Ok(())
    }

    #[inline(never)]
    fn times(&mut self, tag: u8, raw: &[u8]) -> serde_json::Result<()> {
        let times = serde_json::from_slice::<BTreeMap<String, String>>(raw)?;
        for (k, v) in &times {
            let Some(date) = CivilDate::parse(k) else {
                continue;
            };
            let stamp = parse_hebcal_date(v).and_then(|(_, t)| t);
            let day = self.days.entry(date).or_insert(SunDay { date, sunrise: None, sunset: None });
            if tag == SUNRISE {
                day.sunrise = stamp;
            } else {
                day.sunset = stamp;
            }
        }
        Ok(())
    }
}

impl Default for ZmanimParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ZmanimParser {
    pub fn new() -> Self {
        Self { split: Splitter::new(ITEM_LIMIT), sink: ZmanimSink::default() }
    }

    pub fn feed(&mut self, chunk: &[u8]) {
        if self.sink.error.is_none() {
            self.split.feed(chunk, &mut self.sink);
        }
    }

    pub fn finish(mut self) -> serde_json::Result<(Option<WireLocation>, Vec<SunDay>)> {
        self.take()
    }

    /// [`finish`](Self::finish) in place, for a boxed parser.
    pub fn take(&mut self) -> serde_json::Result<(Option<WireLocation>, Vec<SunDay>)> {
        let end = self.split.finish(&mut self.sink);
        if let Some(e) = self.sink.error.take() {
            return Err(e);
        }
        end.map_err(serde_json::Error::custom)?;
        Ok((self.sink.location.take(), core::mem::take(&mut self.sink.days).into_values().collect()))
    }
}

/// Stream-parse a `/zmanim` response from a reader.
#[cfg(feature = "std")]
pub fn parse_zmanim<R: std::io::Read>(reader: R) -> serde_json::Result<(Option<WireLocation>, Vec<SunDay>)> {
    let mut p = ZmanimParser::new();
    read_chunks(reader, |c| p.feed(c))?;
    p.finish()
}

/// One `/complete?q=` city suggestion (for the settings page).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct City {
    pub id: u32,
    pub value: String,
    #[serde(default)]
    pub cc: Option<String>,
}

pub fn parse_complete(body: &[u8]) -> serde_json::Result<Vec<City>> {
    serde_json::from_slice(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::Kind;

    const SAMPLE: &str = r#"{
      "title":"Hebcal New York","date":"2026-10-09T12:00:00.000Z","version":"6.14.1",
      "location":{"title":"New York, NY","city":"New York","tzid":"America/New_York",
                  "latitude":40.71427,"longitude":-74.00597,"cc":"US","geonameid":5128581},
      "range":{"start":"2026-10-01","end":"2026-10-15"},
      "items":[
        {"title":"Candle lighting: 7:15pm","date":"2026-10-03T19:15:00-04:00","category":"candles","title_orig":"Candle lighting","hebrew":"x","memo":"Shmini Atzeret"},
        {"title":"Simchat Torah","date":"2026-10-04","hdate":"23 Tishrei 5787","category":"holiday","subcat":"major","yomtov":true,"memo":"long text"},
        {"title":"Latest Kiddush Levana","date":"2026-10-05T05:00:00-04:00","category":"zmanim","subcat":"kiddushLevana"},
        {"title":"Rosh Chodesh Cheshvan","date":"2026-11-11","category":"roshchodesh"},
        {"title":"Parashat Lech-Lecha","date":"2026-11-14","category":"parashat"}
      ]}"#;

    #[test]
    fn streams_and_filters() {
        let ev =
            parse_events(SAMPLE.as_bytes(), Policy { detail_until: Some(CivilDate::new(2026, 10, 30)), range: None })
                .unwrap();
        assert_eq!(ev.location.tzid.as_deref(), Some("America/New_York"));
        assert_eq!(ev.location.geonameid, Some(5128581));
        assert_eq!(ev.location.latitude.as_deref(), Some("40.71427"));
        assert_eq!(ev.location.longitude.as_deref(), Some("-74.00597"));
        let kinds: Vec<Kind> = ev.items.iter().map(|i| i.kind).collect();
        // Kiddush Levana dropped; parsha beyond the detail window dropped.
        assert_eq!(kinds, vec![Kind::Candles, Kind::YomTov, Kind::RoshChodesh]);
    }

    #[test]
    fn zmanim() {
        let z = r#"{"date":{"start":"2026-10-09","end":"2026-10-09"},"version":"v1.11.3",
            "location":{"tzid":"America/New_York","latitude":40.71427,"longitude":-74.00597},
            "times":{"chatzotNight":{"2026-10-09":"2026-10-09T00:51:00-04:00"},"seaLevelSunrise":{},
                     "sunrise":{"2026-10-09":"2026-10-09T07:04:00-04:00"},
                     "sunset":{"2026-10-09":"2026-10-09T18:25:00-04:00"}}}"#;
        let (loc, sun) = parse_zmanim(z.as_bytes()).unwrap();
        assert_eq!(loc.unwrap().tzid.as_deref(), Some("America/New_York"));
        assert_eq!(sun.len(), 1);
        assert_eq!(sun[0].sunset.unwrap().hm(), (18, 25));
    }

    #[test]
    fn broken_bodies_are_errors() {
        let policy = Policy::default();
        assert!(parse_events(&SAMPLE.as_bytes()[..SAMPLE.len() / 2], policy).is_err());
        assert!(parse_events(&b"<html>502 Bad Gateway</html>"[..], policy).is_err());
        assert!(parse_events(&br#"{"items":[{"title":7}]}"#[..], policy).is_err());
        assert!(parse_zmanim(&br#"{"times":{"sunset":{"2026-10-09":"#[..]).is_err());
    }

    /// The splitter must see exactly what serde's own streaming deserializer
    /// sees, however the body is cut into chunks.
    mod oracle {
        use super::*;
        use core::fmt;
        use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, Visitor};
        use std::path::PathBuf;

        fn parse_events_serde(body: &[u8], policy: Policy) -> serde_json::Result<Events> {
            let mut out = Events::default();
            let mut de = serde_json::Deserializer::from_slice(body);
            EventsSeed { policy, out: &mut out }.deserialize(&mut de)?;
            de.end()?;
            Ok(out)
        }

        struct EventsSeed<'a> {
            policy: Policy,
            out: &'a mut Events,
        }

        impl<'de> DeserializeSeed<'de> for EventsSeed<'_> {
            type Value = ();
            fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
                d.deserialize_map(self)
            }
        }

        impl<'de> Visitor<'de> for EventsSeed<'_> {
            type Value = ();
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a Hebcal events object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "location" => self.out.location = map.next_value::<Option<WireLocation>>()?.unwrap_or_default(),
                        "items" => {
                            for w in map.next_value::<Vec<WireItem>>()? {
                                if let Some(item) = classify(w) {
                                    if self.policy.keep(&item) {
                                        self.out.items.push(item);
                                    }
                                }
                            }
                        }
                        _ => {
                            map.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                Ok(())
            }
        }

        fn parse_zmanim_serde(body: &[u8]) -> (Option<WireLocation>, Vec<SunDay>) {
            #[derive(Deserialize)]
            struct Resp {
                #[serde(default)]
                location: Option<WireLocation>,
                #[serde(default)]
                times: Times,
            }
            #[derive(Deserialize, Default)]
            struct Times {
                #[serde(default)]
                sunrise: BTreeMap<String, String>,
                #[serde(default)]
                sunset: BTreeMap<String, String>,
            }
            let resp: Resp = serde_json::from_slice(body).unwrap();
            let stamp = |s: &String| parse_hebcal_date(s).and_then(|(_, t)| t);
            let mut days: BTreeMap<CivilDate, SunDay> = BTreeMap::new();
            for (k, v) in &resp.times.sunrise {
                let date = CivilDate::parse(k).unwrap();
                days.entry(date).or_insert(SunDay { date, sunrise: None, sunset: None }).sunrise = stamp(v);
            }
            for (k, v) in &resp.times.sunset {
                let date = CivilDate::parse(k).unwrap();
                days.entry(date).or_insert(SunDay { date, sunrise: None, sunset: None }).sunset = stamp(v);
            }
            (resp.location, days.into_values().collect())
        }

        fn fixtures(prefix: &str) -> Vec<(String, Vec<u8>)> {
            let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
            let mut out: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
                .unwrap()
                .map(|e| e.unwrap().path())
                .filter(|p| p.file_name().unwrap().to_str().unwrap().starts_with(prefix))
                .map(|p| (p.display().to_string(), std::fs::read(&p).unwrap()))
                .collect();
            out.sort();
            assert!(!out.is_empty(), "no {prefix} fixtures");
            out
        }

        const CHUNKS: [usize; 5] = [1, 3, 64, 512, usize::MAX];

        #[test]
        fn events_match_serde() {
            for (name, body) in fixtures("events-") {
                for policy in [
                    Policy::default(),
                    Policy {
                        detail_until: CivilDate::parse(&name[name.len() - 9..name.len() - 5].to_string().add("-03-01")),
                        range: None,
                    },
                ] {
                    let want = parse_events_serde(&body, policy).unwrap();
                    assert!(want.items.len() > 50, "{name}");
                    for chunk in CHUNKS {
                        let mut p = EventsParser::new(policy);
                        for c in body.chunks(chunk.min(body.len())) {
                            p.feed(c);
                        }
                        assert_eq!(p.skipped(), 0, "{name}");
                        assert_eq!(p.finish().unwrap(), want, "{name} in chunks of {chunk}");
                    }
                }
            }
        }

        #[test]
        fn zmanim_match_serde() {
            for (name, body) in fixtures("zmanim-") {
                let want = parse_zmanim_serde(&body);
                assert!(!want.1.is_empty(), "{name}");
                for chunk in CHUNKS {
                    let mut p = ZmanimParser::new();
                    for c in body.chunks(chunk.min(body.len())) {
                        p.feed(c);
                    }
                    assert_eq!(p.finish().unwrap(), want, "{name} in chunks of {chunk}");
                }
            }
        }

        trait Add {
            fn add(self, s: &str) -> String;
        }
        impl Add for String {
            fn add(mut self, s: &str) -> String {
                self.push_str(s);
                self
            }
        }
    }
}
