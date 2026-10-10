//! Current weather and a 3-day forecast from Open-Meteo (free, no API key;
//! data CC BY 4.0, credit "Open-Meteo.com").
//!
//! Every number arrives as a JSON float. They are captured as raw JSON text
//! and converted to fixed-point tenths here: serde_json's float parser does
//! not compile for the ESP32 (Xtensa), and tenths are all a display needs.

use crate::prelude::*;
use serde::Deserialize;
use serde_json::value::RawValue;

use crate::hebcal::push_encoded;
use crate::time::CivilDate;

pub const HOST: &str = "https://api.open-meteo.com";

/// Data older than this is not shown.
pub const STALE_AFTER_SECS: i64 = 3 * 3600;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Units {
    /// °C, km/h, hPa.
    #[default]
    Metric,
    /// °C, m/s, mmHg (common in Eastern Europe).
    MetricMs,
    /// °F, mph, inHg.
    Imperial,
}

impl Units {
    pub fn temp(self) -> &'static str {
        match self {
            Units::Imperial => "F",
            _ => "C",
        }
    }

    pub fn wind(self) -> &'static str {
        match self {
            Units::Metric => "KM/H",
            Units::MetricMs => "M/S",
            Units::Imperial => "MPH",
        }
    }

    /// Pressure from tenths of hPa: (number, unit), e.g. ("1020", "HPA").
    pub fn pressure(self, tenths_hpa: i32) -> (String, &'static str) {
        let t = tenths_hpa as i64;
        match self {
            Units::Metric => (div_round(t, 10).to_string(), "HPA"),
            // 1 hPa = 0.750062 mmHg
            Units::MetricMs => (div_round(t * 750_062, 10_000_000).to_string(), "MMHG"),
            // 1 hPa = 0.0295300 inHg; shown with two decimals.
            Units::Imperial => {
                let h = div_round(t * 2953, 10_000);
                (format!("{}.{:02}", h / 100, h % 100), "INHG")
            }
        }
    }
}

/// What the sky is doing, from the WMO weather code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Condition {
    Clear,
    MainlyClear,
    PartlyCloudy,
    Overcast,
    Fog,
    Drizzle,
    Rain,
    FreezingRain,
    Snow,
    Showers,
    SnowShowers,
    Thunder,
}

impl Condition {
    pub const ALL: [Condition; 12] = [
        Condition::Clear,
        Condition::MainlyClear,
        Condition::PartlyCloudy,
        Condition::Overcast,
        Condition::Fog,
        Condition::Drizzle,
        Condition::Rain,
        Condition::FreezingRain,
        Condition::Snow,
        Condition::Showers,
        Condition::SnowShowers,
        Condition::Thunder,
    ];

    pub fn from_wmo(code: i32) -> Self {
        match code {
            0 => Condition::Clear,
            1 => Condition::MainlyClear,
            2 => Condition::PartlyCloudy,
            45 | 48 => Condition::Fog,
            51 | 53 | 55 => Condition::Drizzle,
            56 | 57 | 66 | 67 => Condition::FreezingRain,
            61 | 63 | 65 => Condition::Rain,
            71 | 73 | 75 | 77 => Condition::Snow,
            80..=82 => Condition::Showers,
            85 | 86 => Condition::SnowShowers,
            95 | 96 | 99 => Condition::Thunder,
            _ => Condition::Overcast,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Condition::Clear => "CLEAR",
            Condition::MainlyClear => "MOSTLY CLEAR",
            Condition::PartlyCloudy => "PARTLY CLOUDY",
            Condition::Overcast => "CLOUDY",
            Condition::Fog => "FOG",
            Condition::Drizzle => "DRIZZLE",
            Condition::Rain => "RAIN",
            Condition::FreezingRain => "FREEZING RAIN",
            Condition::Snow => "SNOW",
            Condition::Showers => "SHOWERS",
            Condition::SnowShowers => "SNOW SHOWERS",
            Condition::Thunder => "THUNDERSTORM",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Current {
    /// Tenths of a degree in the chosen unit.
    pub temp: i32,
    pub feels: Option<i32>,
    /// Percent.
    pub humidity: Option<i32>,
    /// Tenths of hPa at sea level.
    pub pressure: Option<i32>,
    /// Tenths, in the chosen wind unit.
    pub wind: Option<i32>,
    /// Degrees the wind comes from.
    pub wind_dir: Option<i32>,
    pub condition: Condition,
    pub is_day: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Day {
    pub date: CivilDate,
    pub condition: Condition,
    pub max: Option<i32>,
    pub min: Option<i32>,
    /// Highest chance of precipitation that day, percent.
    pub precip: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Weather {
    pub units: Units,
    pub current: Current,
    pub days: Vec<Day>,
    pub fetched_at: i64,
}

impl Weather {
    pub fn is_stale(&self, now: i64) -> bool {
        now - self.fetched_at > STALE_AFTER_SECS
    }

    /// `5 KM/H W`
    pub fn wind_text(&self) -> Option<String> {
        let w = div_round(self.current.wind? as i64, 10);
        let dir = self.current.wind_dir.map_or("", compass);
        Some(format!("{w} {} {dir}", self.units.wind()).trim_end().to_string())
    }
}

/// Whole degrees from tenths, rounded half away from zero.
pub fn deg(tenths: i32) -> i32 {
    div_round(tenths as i64, 10) as i32
}

fn div_round(n: i64, d: i64) -> i64 {
    if n >= 0 {
        (n + d / 2) / d
    } else {
        -((-n + d / 2) / d)
    }
}

/// 8-point compass for the direction the wind blows from.
pub fn compass(deg: i32) -> &'static str {
    const POINTS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    POINTS[((deg.rem_euclid(360) + 22) % 360 / 45) as usize]
}

/// `"12.4"` → 124, `"-3"` → -30, `"1020.35"` → 10204; `None` for `null` or
/// anything that is not a plain decimal.
pub fn tenths(raw: &str) -> Option<i32> {
    let s = raw.trim();
    let (neg, s) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let (int, frac) = s.split_once('.').unwrap_or((s, ""));
    if int.is_empty() || !int.bytes().all(|b| b.is_ascii_digit()) || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut v: i64 = int.parse::<i64>().ok()? * 10;
    let mut digits = frac.bytes();
    if let Some(d) = digits.next() {
        v += (d - b'0') as i64;
    }
    if digits.next().is_some_and(|d| d >= b'5') {
        v += 1;
    }
    i32::try_from(if neg { -v } else { v }).ok()
}

pub fn url(lat: &str, lon: &str, units: Units) -> String {
    let mut u = format!("{HOST}/v1/forecast?latitude=");
    push_encoded(&mut u, lat);
    u.push_str("&longitude=");
    push_encoded(&mut u, lon);
    u.push_str(
        "&current=temperature_2m,apparent_temperature,relative_humidity_2m,pressure_msl,\
         wind_speed_10m,wind_direction_10m,weather_code,is_day\
         &daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max\
         &forecast_days=3&timezone=auto",
    );
    u.push_str(match units {
        Units::Metric => "&wind_speed_unit=kmh",
        Units::MetricMs => "&wind_speed_unit=ms",
        Units::Imperial => "&temperature_unit=fahrenheit&wind_speed_unit=mph",
    });
    u
}

type Num = Option<Box<RawValue>>;

#[derive(Deserialize)]
struct Resp {
    #[serde(default)]
    current: Option<WireCurrent>,
    #[serde(default)]
    daily: Option<WireDaily>,
}

#[derive(Deserialize)]
struct WireCurrent {
    #[serde(default)]
    temperature_2m: Num,
    #[serde(default)]
    apparent_temperature: Num,
    #[serde(default)]
    relative_humidity_2m: Num,
    #[serde(default)]
    pressure_msl: Num,
    #[serde(default)]
    wind_speed_10m: Num,
    #[serde(default)]
    wind_direction_10m: Num,
    #[serde(default)]
    weather_code: Num,
    #[serde(default)]
    is_day: Num,
}

#[derive(Deserialize)]
struct WireDaily {
    #[serde(default)]
    time: Vec<String>,
    #[serde(default)]
    weather_code: Vec<Box<RawValue>>,
    #[serde(default)]
    temperature_2m_max: Vec<Box<RawValue>>,
    #[serde(default)]
    temperature_2m_min: Vec<Box<RawValue>>,
    #[serde(default)]
    precipitation_probability_max: Vec<Box<RawValue>>,
}

fn num(n: &Num) -> Option<i32> {
    n.as_deref().and_then(|r| tenths(r.get()))
}

fn at(v: &[Box<RawValue>], i: usize) -> Option<i32> {
    v.get(i).and_then(|r| tenths(r.get()))
}

/// Parse a forecast response read from `reader` (~1 KB).
#[cfg(feature = "std")]
pub fn parse<R: std::io::Read>(mut reader: R, units: Units, fetched_at: i64) -> serde_json::Result<Option<Weather>> {
    let mut body = Vec::new();
    reader.read_to_end(&mut body).map_err(serde_json::Error::io)?;
    parse_slice(&body, units, fetched_at)
}

/// Parse a forecast response. `None` when it has no current temperature.
pub fn parse_slice(body: &[u8], units: Units, fetched_at: i64) -> serde_json::Result<Option<Weather>> {
    let resp: Resp = serde_json::from_slice(body)?;
    let Some(c) = resp.current else {
        return Ok(None);
    };
    let Some(temp) = num(&c.temperature_2m) else {
        return Ok(None);
    };
    let whole = |t: Option<i32>| t.map(deg);
    let current = Current {
        temp,
        feels: num(&c.apparent_temperature),
        humidity: whole(num(&c.relative_humidity_2m)),
        pressure: num(&c.pressure_msl),
        wind: num(&c.wind_speed_10m),
        wind_dir: whole(num(&c.wind_direction_10m)),
        condition: Condition::from_wmo(whole(num(&c.weather_code)).unwrap_or(3)),
        is_day: whole(num(&c.is_day)) != Some(0),
    };
    let mut days = Vec::new();
    if let Some(d) = resp.daily {
        for (i, date) in d.time.iter().enumerate() {
            let Some(date) = CivilDate::parse(date) else {
                continue;
            };
            days.push(Day {
                date,
                condition: Condition::from_wmo(whole(at(&d.weather_code, i)).unwrap_or(3)),
                max: at(&d.temperature_2m_max, i),
                min: at(&d.temperature_2m_min, i),
                precip: whole(at(&d.precipitation_probability_max, i)),
            });
        }
    }
    Ok(Some(Weather { units, current, days, fetched_at }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_point() {
        assert_eq!(tenths("12.4"), Some(124));
        assert_eq!(tenths("-3"), Some(-30));
        assert_eq!(tenths("-0.25"), Some(-3));
        assert_eq!(tenths("1020.35"), Some(10204));
        assert_eq!(tenths("null"), None);
        assert_eq!(tenths("1e3"), None);
        assert_eq!((deg(124), deg(125), deg(-125), deg(-124)), (12, 13, -13, -12));
        assert_eq!(compass(289), "W");
        assert_eq!(compass(350), "N");
        assert_eq!(compass(-45), "NW");
    }

    #[test]
    fn units() {
        assert_eq!(Units::Metric.pressure(10203), ("1020".into(), "HPA"));
        assert_eq!(Units::MetricMs.pressure(10203), ("765".into(), "MMHG"));
        assert_eq!(Units::Imperial.pressure(10203), ("30.13".into(), "INHG"));
    }

    #[test]
    fn parses_open_meteo() {
        let json = r#"{"latitude":40.710335,"longitude":-73.99308,"elevation":51.0,
          "current_units":{"temperature_2m":"°C"},
          "current":{"time":"2026-10-09T08:30","interval":900,"temperature_2m":12.4,"apparent_temperature":11.2,
            "relative_humidity_2m":75,"pressure_msl":1020.3,"wind_speed_10m":4.7,"wind_direction_10m":289,
            "weather_code":0,"is_day":1},
          "daily":{"time":["2026-10-09","2026-10-10","2026-10-11"],"weather_code":[3,3,81],
            "temperature_2m_max":[22.4,21.3,17.3],"temperature_2m_min":[10.7,8.7,12.9],
            "precipitation_probability_max":[0,null,80]}}"#;
        let w = parse(json.as_bytes(), Units::Metric, 0).unwrap().unwrap();
        assert_eq!(w.current.temp, 124);
        assert_eq!(w.current.humidity, Some(75));
        assert_eq!(w.current.condition, Condition::Clear);
        assert!(w.current.is_day);
        assert_eq!(w.wind_text().as_deref(), Some("5 KM/H W"));
        assert_eq!(w.days.len(), 3);
        assert_eq!(w.days[2].condition, Condition::Showers);
        assert_eq!((w.days[1].precip, w.days[2].precip), (None, Some(80)));
        assert_eq!(w.days[0].max, Some(224));
    }
}
