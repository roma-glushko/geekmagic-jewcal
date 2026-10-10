//! Fetch the calendar from Hebcal and build the model.

use std::io::BufReader;

use jewcal_model::calendar::CalendarModel;
use jewcal_model::hebcal::{self, Window};
use jewcal_model::time::CivilDate;
use jewcal_model::weather::{self, Units, Weather};
use jewcal_model::wire::{self, Policy};

use crate::httpc;
use crate::settings::Settings;

/// ~40 KB is typical for 200 days; the cap only guards against garbage.
const MAX_EVENTS: usize = 128 * 1024;
const MAX_ZMANIM: usize = 64 * 1024;

pub fn fetch(s: &Settings, today: CivilDate, now: i64) -> anyhow::Result<CalendarModel> {
    let q = s.query().ok_or_else(|| anyhow::anyhow!("no location set"))?;
    let w = Window::around(today);
    let policy = Policy { detail_until: Some(w.detail_until), range: None };
    let events = httpc::get(&hebcal::events_url(&q, w.start, w.end), MAX_EVENTS, |r| {
        Ok(wire::parse_events(BufReader::with_capacity(512, r), policy)?)
    })?;
    log::info!("hebcal: {} items for {:?} (tz {:?})", events.items.len(), events.location.title, events.location.tzid);
    // Sunrise/sunset only drive the Hebrew-date rollover and the sky
    // colours: not worth losing the calendar over.
    let sun = match httpc::get(&hebcal::zmanim_url(&q.place, w.sun_start, w.sun_end), MAX_ZMANIM, |r| {
        Ok(wire::parse_zmanim(BufReader::with_capacity(512, r))?.1)
    }) {
        Ok(sun) => sun,
        Err(e) => {
            log::warn!("zmanim fetch failed: {e:#}");
            Vec::new()
        }
    };
    Ok(CalendarModel::build(events, sun, (w.start, w.end), q.force_israel, now))
}

/// Current weather and 3 days ahead from Open-Meteo (~1 KB).
pub fn weather(lat: &str, lon: &str, units: Units, now: i64) -> anyhow::Result<Option<Weather>> {
    httpc::get(&weather::url(lat, lon, units), 8 * 1024, |r| {
        Ok(weather::parse(BufReader::with_capacity(512, r), units, now)?)
    })
}

/// City autocomplete for the settings page: Hebcal's JSON, passed through.
pub fn search_cities(prefix: &str) -> anyhow::Result<String> {
    httpc::get(&hebcal::complete_url(prefix), 16 * 1024, |r| {
        let mut body = String::new();
        r.read_to_string(&mut body)?;
        Ok(body)
    })
}
