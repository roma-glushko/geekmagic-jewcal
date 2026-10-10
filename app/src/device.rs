//! The SmallTV Ultra's main loop (ESP8266: one thread, ~40 KB of heap).
//!
//! The C++ firmware does the I/O. It asks [`Device::next_request`] what to
//! fetch, streams the HTTP body into [`Device::feed`], reports how the
//! request ended with [`Device::finish`], and calls [`Device::frame`] a few
//! times a second to draw. The same decisions as the SmallTV Pro's threads:
//! calendar at boot, on settings changes and daily around 03:00 with
//! backoff on errors; weather every half hour.
//!
//! Memory: two calendars do not fit at once. A refresh drops the old model
//! before the new events arrive and leaves the last frame on the panel until
//! the new one is built. A failed refresh shows the error and retries with
//! backoff. Frames stay small too: the loop stack is 4 KB, shared with the
//! web server, so the parsers live on the heap and each kind of response is
//! finished in its own function.

use crate::prelude::*;

use jewcal_model::calendar::CalendarModel;
use jewcal_model::hebcal::{self, Place, Query, Window};
use jewcal_model::snapshot::Snapshot;
use jewcal_model::time::month_short;
use jewcal_model::tz::Tz;
use jewcal_model::weather::{self, Units, Weather};
use jewcal_model::wire::{Events, EventsParser, Policy, SunDay, ZmanimParser};
use jewcal_ui::{palette, Canvas, Ctx, Scheduler, Status};

use crate::settings::Settings;
use crate::web::{self, Applied, PageStatus};

/// The settings page is also reachable as `http://jewcal.local/` (mDNS).
pub const HOSTNAME: &str = "jewcal";
const RETRY_MIN_S: i64 = 60;
const RETRY_MAX_S: i64 = 3600;
/// Open-Meteo updates every 15 minutes; half an hour is plenty for a desk.
const WEATHER_EVERY_S: i64 = 30 * 60;
const WEATHER_RETRY_S: i64 = 10 * 60;
/// Open-Meteo's answer is ~1.5 KB; anything much larger is not a forecast.
const WEATHER_MAX: usize = 6 * 1024;
/// Before SNTP answers, `now` is a few seconds after 1970.
const CLOCK_VALID_AFTER: i64 = 1_700_000_000;

/// What [`Device::next_request`] asks the firmware to GET.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Request {
    Events = 1,
    Zmanim = 2,
    Weather = 3,
}

/// How an HTTP request ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Status 200 and the whole body was fed.
    Ok,
    /// Any other status; `retry_after` from the header of a 429/503, seconds.
    Status { code: u16, retry_after: Option<u32> },
    /// No connection, a timeout, or the body was cut short.
    Network,
}

/// A calendar together with the settings key it was fetched under.
struct Loaded {
    key: String,
    model: CalendarModel,
}

/// A calendar refresh: events first, then sunrise/sunset.
struct Refresh {
    key: String,
    window: Window,
    force_israel: bool,
}

enum Pending {
    Events(Refresh, Box<EventsParser>),
    Zmanim(Refresh, Events, Box<ZmanimParser>),
    Weather { key: String, units: Units, body: Vec<u8>, overflow: bool },
}

pub struct Device {
    settings: Settings,
    /// Changed by the device itself (time zone learned): persist it.
    settings_dirty: bool,
    calendar: Option<Loaded>,
    weather: Option<Weather>,
    ip: Option<String>,
    last_fetch: Option<i64>,
    error: Option<String>,

    next_at: i64,
    backoff: i64,
    wake: bool,
    weather_at: i64,
    weather_key: String,
    /// Events received, sunrise/sunset not yet requested.
    queued: Option<(Refresh, Events)>,
    pending: Option<Pending>,
    /// The calendar on screen was dropped for a refresh of the same
    /// settings: keep its last frame until the new one is built.
    replacing: bool,
    rng: u32,

    sched: Scheduler,
    frame: u32,
    tz: (String, Tz),
}

impl Device {
    /// `seed` spreads the daily refresh of many devices over half an hour.
    pub fn new(settings: Settings, now_ms: u64, seed: u32) -> Self {
        let sched = Scheduler::new(settings.cycle_min as u32, settings.dwell_s as u32, now_ms);
        Self {
            settings,
            settings_dirty: false,
            calendar: None,
            weather: None,
            ip: None,
            last_fetch: None,
            error: None,
            next_at: 0,
            backoff: 0,
            wake: true,
            weather_at: 0,
            weather_key: String::new(),
            queued: None,
            pending: None,
            replacing: false,
            rng: seed | 1,
            sched,
            frame: 0,
            tz: (String::new(), Tz::UTC),
        }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Whether the settings changed since the last call (persist them).
    pub fn take_settings_dirty(&mut self) -> bool {
        core::mem::take(&mut self.settings_dirty)
    }

    /// The station address, or `None` while offline (setup mode, no WiFi).
    pub fn set_ip(&mut self, ip: Option<String>) {
        self.ip = ip;
    }

    pub fn has_calendar(&self) -> bool {
        self.current().is_some()
    }

    pub fn weather(&self) -> Option<&Weather> {
        self.weather.as_ref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn fetching(&self) -> bool {
        self.pending.is_some() || self.queued.is_some()
    }

    /// The calendar for the current settings, if one is loaded.
    fn current(&self) -> Option<&Loaded> {
        let key = self.settings.fetch_key();
        self.calendar.as_ref().filter(|c| c.key == key)
    }

    fn zone(&self) -> Tz {
        Tz::for_tzid(&self.settings.tzid).unwrap_or(Tz::UTC)
    }

    // ── Fetching ─────────────────────────────────────────────────────────────

    /// The next GET to make, if one is due. Its body goes to [`feed`](Self::feed),
    /// then [`finish`](Self::finish) must be called exactly once.
    pub fn next_request(&mut self, now: i64, clock_valid: bool) -> Option<(Request, String)> {
        // A request the firmware never finished is abandoned.
        self.pending = None;
        if let Some((refresh, events)) = self.queued.take() {
            let mut place = self.settings.place()?;
            // Coordinates need a zone for /zmanim ("Timezone required");
            // before one is saved, use the one Hebcal just reported.
            if let Place::Coords { tzid: tz @ None, .. } = &mut place {
                tz.clone_from(&events.location.tzid);
            }
            let url = plain_http(hebcal::zmanim_url(&place, refresh.window.sun_start, refresh.window.sun_end));
            self.pending = Some(Pending::Zmanim(refresh, events, Box::new(ZmanimParser::new())));
            return Some((Request::Zmanim, url));
        }
        if !clock_valid {
            return None;
        }
        let key = self.settings.fetch_key();
        if let Some(q) = self.settings.query() {
            // A failed refresh waits for `next_at` (backoff), even with no
            // calendar at all; new settings set `wake`.
            let other_settings = self.calendar.as_ref().is_some_and(|c| c.key != key);
            if self.wake || other_settings || now >= self.next_at {
                return Some(self.start_refresh(q, key, now));
            }
        }
        if let Some((lat, lon)) = self.coordinates() {
            let units = self.settings.units();
            let key = format!("{lat},{lon},{}", self.settings.units);
            if now >= self.weather_at || key != self.weather_key {
                self.weather_key = key.clone();
                // Until it answers, do not ask again on every loop.
                self.weather_at = now + WEATHER_RETRY_S;
                let url = plain_http(weather::url(&lat, &lon, units));
                self.pending = Some(Pending::Weather { key, units, body: Vec::new(), overflow: false });
                return Some((Request::Weather, url));
            }
        }
        None
    }

    #[inline(never)]
    fn start_refresh(&mut self, q: Query, key: String, now: i64) -> (Request, String) {
        self.wake = false;
        // No room for two calendars: drop the old one now.
        self.replacing = self.calendar.take().is_some_and(|c| c.key == key);
        let window = Window::around(self.zone().local(now).date);
        let url = plain_http(hebcal::events_url(&q, window.start, window.end));
        let policy = Policy { detail_until: Some(window.detail_until), range: None };
        let refresh = Refresh { key, window, force_israel: q.force_israel };
        self.pending = Some(Pending::Events(refresh, Box::new(EventsParser::new(policy))));
        (Request::Events, url)
    }

    /// Drop the request just handed out without making it (the firmware
    /// could not); it counts as a failure, so it is not retried at once.
    pub fn cancel(&mut self, now: i64) {
        match self.pending.take() {
            Some(Pending::Weather { .. }) => self.weather_at = now + WEATHER_RETRY_S,
            Some(_) => self.fail(now, "request too long".into(), None),
            None => {}
        }
    }

    /// Part of the body of the request in flight.
    pub fn feed(&mut self, chunk: &[u8]) {
        match &mut self.pending {
            Some(Pending::Events(_, p)) => p.feed(chunk),
            Some(Pending::Zmanim(_, _, p)) => p.feed(chunk),
            Some(Pending::Weather { body, overflow, .. }) => {
                if body.len() + chunk.len() > WEATHER_MAX {
                    *overflow = true;
                } else if !*overflow {
                    body.extend_from_slice(chunk);
                }
            }
            None => {}
        }
    }

    /// The request in flight is over.
    pub fn finish(&mut self, outcome: Outcome, now: i64) {
        // One function per kind, never inlined: only the running one's
        // locals are on the 4 KB stack, under the model build.
        match self.pending.take() {
            Some(Pending::Events(refresh, parser)) => self.finish_events(refresh, parser, outcome, now),
            Some(Pending::Zmanim(refresh, events, parser)) => self.finish_zmanim(refresh, events, parser, outcome, now),
            Some(Pending::Weather { key, units, body, overflow }) => {
                self.finish_weather(key, units, body, overflow, outcome, now)
            }
            None => {}
        }
    }

    #[inline(never)]
    fn finish_zmanim(
        &mut self,
        refresh: Refresh,
        events: Events,
        mut parser: Box<ZmanimParser>,
        outcome: Outcome,
        now: i64,
    ) {
        // Sunrise/sunset only drive the Hebrew-date rollover and the sky
        // colours: not worth losing the calendar over.
        let sun = match outcome {
            Outcome::Ok => parser.take().map(|(_, sun)| sun).unwrap_or_default(),
            _ => Vec::new(),
        };
        drop(parser);
        self.install_calendar(refresh, events, sun, now);
    }

    #[inline(never)]
    fn finish_weather(&mut self, key: String, units: Units, body: Vec<u8>, overflow: bool, outcome: Outcome, now: i64) {
        let parsed = match outcome {
            Outcome::Ok if !overflow => weather::parse_slice(&body, units, now).ok().flatten(),
            _ => None,
        };
        drop(body);
        match parsed {
            Some(w) if key == self.weather_key => {
                self.weather = Some(w);
                self.weather_at = now + WEATHER_EVERY_S;
            }
            _ => self.weather_at = now + WEATHER_RETRY_S,
        }
    }

    #[inline(never)]
    fn finish_events(&mut self, refresh: Refresh, mut parser: Box<EventsParser>, outcome: Outcome, now: i64) {
        match outcome {
            Outcome::Ok => match parser.take() {
                Ok(events) => self.queued = Some((refresh, events)),
                Err(_) => self.fail(now, "bad answer from hebcal.com".into(), None),
            },
            Outcome::Status { code: 404, .. } => self.fail(now, "hebcal.com does not know this city".into(), None),
            Outcome::Status { code, retry_after } => self.fail(now, format!("hebcal.com: HTTP {code}"), retry_after),
            Outcome::Network => self.fail(now, "cannot reach hebcal.com".into(), None),
        }
    }

    #[inline(never)]
    fn install_calendar(&mut self, refresh: Refresh, events: Events, sun: Vec<SunDay>, now: i64) {
        let window = (refresh.window.start, refresh.window.end);
        let model = CalendarModel::build(events, sun, window, refresh.force_israel, now);
        if let Some(tzid) = &model.location.tzid {
            if self.settings.tzid != *tzid && self.settings.fetch_key() == refresh.key {
                self.settings.tzid.clone_from(tzid);
                self.settings_dirty = true;
            }
        }
        self.calendar = Some(Loaded { key: refresh.key, model });
        self.replacing = false;
        self.last_fetch = Some(now);
        self.error = None;
        self.backoff = 0;
        self.next_at = self.next_daily_refresh(now);
    }

    fn fail(&mut self, now: i64, error: String, retry_after: Option<u32>) {
        self.queued = None;
        self.replacing = false;
        self.error = Some(error);
        self.backoff = match retry_after {
            Some(s) => s as i64,
            None => (self.backoff * 2).max(RETRY_MIN_S),
        }
        .min(RETRY_MAX_S);
        self.next_at = now + self.backoff;
    }

    /// Next 03:00 local, plus up to half an hour of jitter so devices do not
    /// all hit Hebcal at once.
    fn next_daily_refresh(&mut self, now: i64) -> i64 {
        let tz = self.zone();
        let local = tz.local(now);
        let day = if local.h >= 3 { local.date.add_days(1) } else { local.date };
        let target = day.days() * 86_400 + 3 * 3600 - tz.offset_at(now) as i64;
        // xorshift32
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        target + (self.rng % 1800) as i64
    }

    /// Where to ask for weather: the coordinates typed in settings, else the
    /// ones Hebcal reported for the chosen city (kept as text, never floats).
    fn coordinates(&self) -> Option<(String, String)> {
        let s = &self.settings;
        if !s.lat.is_empty() && !s.lon.is_empty() {
            return Some((s.lat.clone(), s.lon.clone()));
        }
        let loc = &self.current()?.model.location;
        Some((loc.latitude.clone()?, loc.longitude.clone()?))
    }

    /// The settings page's city search, through Open-Meteo's geocoder: Hebcal's
    /// `/complete` only answers over HTTPS. Its ids are GeoNames ids, as
    /// Hebcal's are; the page keeps only populated places of 1,000+ people
    /// (what Hebcal knows), see `web::SEARCH_JS`.
    pub fn city_url(q: &str) -> Option<String> {
        let q = q.trim();
        if !(2..=40).contains(&q.len()) {
            return None;
        }
        let mut url =
            String::from("http://geocoding-api.open-meteo.com/v1/search?count=10&language=en&format=json&name=");
        hebcal::push_encoded(&mut url, q);
        Some(url)
    }

    // ── Web ──────────────────────────────────────────────────────────────────

    pub fn settings_page(&self, out: &mut impl core::fmt::Write, notice: &str, extras: &str) -> core::fmt::Result {
        let st = PageStatus { error: self.error.as_deref(), fetching: self.fetching() };
        web::settings_page(out, &self.settings, &st, notice, extras)
    }

    /// Apply the settings form; the caller persists [`settings`](Self::settings)
    /// on success.
    pub fn apply_form(&mut self, field: impl Fn(&str) -> Option<String>) -> Result<Applied, String> {
        let applied = web::apply_form(&mut self.settings, field)?;
        if applied.refetch {
            self.wake = true;
            self.error = None;
        }
        Ok(applied)
    }

    // ── Drawing ──────────────────────────────────────────────────────────────

    /// Draw the current screen; returns the backlight level in percent.
    pub fn frame(&mut self, canvas: &mut Canvas, now: i64, now_ms: u64) -> u8 {
        let s = &self.settings;
        self.sched.set_timing(s.cycle_min as u32, s.dwell_s as u32, now_ms);
        let key = s.fetch_key();
        let loaded = self.calendar.as_ref().filter(|c| c.key == key);
        let tzid = if s.tzid.is_empty() {
            loaded.and_then(|c| c.model.location.tzid.clone()).unwrap_or_default()
        } else {
            s.tzid.clone()
        };
        if tzid != self.tz.0 {
            let tz = Tz::for_tzid(&tzid).unwrap_or(Tz::UTC);
            self.tz = (tzid, tz);
        }
        let tz = &self.tz.1;
        // Night hours mean nothing before the clock is set (night brightness
        // may be 0: a black screen until SNTP answers).
        let level = if now < CLOCK_VALID_AFTER { s.bright_day } else { s.brightness(tz.local(now).h) };
        if loaded.is_none() && self.replacing && self.fetching() {
            return level;
        }
        let url = self.ip.as_ref().map(|ip| format!("http://{ip}/"));
        let status = Status {
            url: url.clone(),
            host: self.ip.as_ref().map(|_| format!("{HOSTNAME}.local")),
            location: (!s.place_name.is_empty()).then(|| s.place_name.clone()),
            israel: s.force_israel || s.cc == "IL" || loaded.is_some_and(|c| c.model.israel),
            updated: self.last_fetch.map(|t| {
                let l = tz.local(t);
                format!("{} {} {:02}:{:02}", month_short(l.date.m), l.date.d, l.h, l.m)
            }),
            error: self.error.clone(),
        };
        let weather = self.weather.as_ref();
        match loaded {
            Some(loaded) => {
                let snap = Snapshot::at(&loaded.model, tz, now);
                let avail = jewcal_ui::available(&snap, weather);
                let screen = self.sched.tick(now_ms, &avail);
                let ctx = Ctx { frame: self.frame, status: &status, weather };
                jewcal_ui::draw(screen, canvas, &snap, &ctx);
            }
            None => {
                let (title, line) = if s.place().is_none() {
                    ("SET LOCATION", format!("open {}", url.as_deref().unwrap_or("the settings page")))
                } else if let (Some(e), false) = (&self.error, self.fetching()) {
                    ("NO CALENDAR", e.clone())
                } else {
                    ("LOADING", "hebcal.com".to_string())
                };
                jewcal_ui::draw_message(canvas, title, &[&line], palette::YELLOW, self.frame);
            }
        }
        self.frame = self.frame.wrapping_add(1);
        level
    }
}

/// The ESP8266 has no room for TLS next to the display and the web server;
/// Hebcal and Open-Meteo both serve plain HTTP.
fn plain_http(url: String) -> String {
    match url.strip_prefix("https://") {
        Some(rest) => format!("http://{rest}"),
        None => url,
    }
}
