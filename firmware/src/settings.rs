//! User settings ([`jewcal_app::Settings`]), persisted in NVS (namespace
//! `jewcal`).

use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault};
pub use jewcal_app::Settings;

const NS: &str = "jewcal";

fn nvs(part: &EspDefaultNvsPartition) -> anyhow::Result<EspNvs<NvsDefault>> {
    Ok(EspNvs::new(part.clone(), NS, true)?)
}

fn get_str(nvs: &EspNvs<NvsDefault>, key: &str) -> anyhow::Result<String> {
    let mut buf = [0u8; 160];
    Ok(nvs.get_str(key, &mut buf)?.unwrap_or_default().to_string())
}

fn get_u8(nvs: &EspNvs<NvsDefault>, key: &str, default: u8) -> anyhow::Result<u8> {
    Ok(nvs.get_u8(key)?.unwrap_or(default))
}

pub fn load(part: &EspDefaultNvsPartition) -> anyhow::Result<Settings> {
    let n = nvs(part)?;
    let d = Settings::default();
    Ok(Settings {
        ssid: get_str(&n, "ssid")?,
        pass: get_str(&n, "pass")?,
        geonameid: n.get_u32("geo")?.filter(|&g| g > 0),
        lat: get_str(&n, "lat")?,
        lon: get_str(&n, "lon")?,
        place_name: get_str(&n, "name")?,
        cc: get_str(&n, "cc")?,
        tzid: get_str(&n, "tzid")?,
        candle_minutes: get_u8(&n, "candle", d.candle_minutes)?,
        havdalah_minutes: get_u8(&n, "havdalah", d.havdalah_minutes)?,
        force_israel: get_u8(&n, "israel", 0)? == 1,
        modern: get_u8(&n, "modern", d.modern as u8)? == 1,
        minor: get_u8(&n, "minor", d.minor as u8)? == 1,
        cycle_min: get_u8(&n, "cycle", d.cycle_min)?,
        dwell_s: get_u8(&n, "dwell", d.dwell_s)?,
        bright_day: get_u8(&n, "bday", d.bright_day)?,
        bright_night: get_u8(&n, "bnight", d.bright_night)?,
        night_from: get_u8(&n, "nfrom", d.night_from)?,
        night_to: get_u8(&n, "nto", d.night_to)?,
        units: get_u8(&n, "units", d.units)?,
    })
}

/// Persist everything except the WiFi credentials (see [`save_wifi`]).
pub fn save(part: &EspDefaultNvsPartition, s: &Settings) -> anyhow::Result<()> {
    let n = nvs(part)?;
    n.set_u32("geo", s.geonameid.unwrap_or(0))?;
    n.set_str("lat", &s.lat)?;
    n.set_str("lon", &s.lon)?;
    n.set_str("name", &s.place_name)?;
    n.set_str("cc", &s.cc)?;
    n.set_str("tzid", &s.tzid)?;
    n.set_u8("candle", s.candle_minutes)?;
    n.set_u8("havdalah", s.havdalah_minutes)?;
    n.set_u8("israel", s.force_israel as u8)?;
    n.set_u8("modern", s.modern as u8)?;
    n.set_u8("minor", s.minor as u8)?;
    n.set_u8("cycle", s.cycle_min)?;
    n.set_u8("dwell", s.dwell_s)?;
    n.set_u8("bday", s.bright_day)?;
    n.set_u8("bnight", s.bright_night)?;
    n.set_u8("nfrom", s.night_from)?;
    n.set_u8("nto", s.night_to)?;
    n.set_u8("units", s.units)?;
    Ok(())
}

pub fn save_wifi(part: &EspDefaultNvsPartition, ssid: &str, pass: &str) -> anyhow::Result<()> {
    let n = nvs(part)?;
    n.set_str("ssid", ssid)?;
    n.set_str("pass", pass)?;
    Ok(())
}

pub fn clear_wifi(part: &EspDefaultNvsPartition) -> anyhow::Result<()> {
    let n = nvs(part)?;
    n.remove("ssid")?;
    n.remove("pass")?;
    Ok(())
}
