//! The settings page (location search, times, display, live screenshot)
//! and the first-run WiFi page, shared by both firmwares. Pages are written
//! to any [`fmt::Write`]: a `String` on the ESP32, the HTTP response in small
//! chunks on the ESP8266.

use crate::prelude::*;
use core::fmt::{self, Write};

use jewcal_ui::canvas::{H, W};
use jewcal_ui::palette::SWEETIE16;

use crate::settings::Settings;

pub const CSS: &str = r#"<style>
:root{--bg:#1a1c2c;--panel:#333c57;--text:#f4f4f4;--dim:#94b0c2;--gold:#ffcd75;--sky:#41a6f6;--red:#b13e53;--green:#38b764}
*{box-sizing:border-box}
body{background:var(--bg);color:var(--text);font:15px/1.5 ui-monospace,Menlo,Consolas,monospace;max-width:34rem;margin:0 auto;padding:1rem}
h1{color:var(--gold);font-size:1.25rem;letter-spacing:.15em;margin:.2rem 0 1rem}
.screen{display:block;width:240px;height:240px;image-rendering:pixelated;border:4px solid var(--panel);margin:0 auto 1rem}
fieldset{border:2px solid var(--panel);margin:0 0 1rem;padding:.6rem .8rem}
legend{color:var(--gold);padding:0 .4rem;text-transform:uppercase;letter-spacing:.1em}
label{display:block;margin:.35rem 0}
input,select{background:var(--panel);color:var(--text);border:2px solid #566c86;font:inherit;padding:.3rem .45rem}
input[type=number]{width:4.5rem}input[type=text],#q{width:100%}
button{background:var(--gold);color:var(--bg);border:0;font:inherit;font-weight:bold;padding:.6rem 1rem;width:100%;cursor:pointer;text-transform:uppercase;letter-spacing:.1em}
#res button{background:var(--panel);color:var(--text);text-align:left;margin:.2rem 0;text-transform:none;letter-spacing:0;font-weight:normal}
.cur{color:var(--sky)}.dim{color:var(--dim);font-size:.85em}.ok{color:var(--green)}.err{color:var(--red)}
a{color:var(--sky)}
</style>"#;

/// City search. `/cities` answers with Hebcal's `/complete` list (SmallTV
/// Pro) or Open-Meteo's geocoder (SmallTV Ultra, plain HTTP only), whose
/// populated places of 1,000+ people are the ones Hebcal knows by id.
pub const SEARCH_JS: &str = r#"<script>
const q=document.getElementById('q'),res=document.getElementById('res');let t;
q.addEventListener('input',()=>{clearTimeout(t);t=setTimeout(async()=>{
 if(q.value.trim().length<2){res.innerHTML='';return}
 try{const r=await fetch('/cities?q='+encodeURIComponent(q.value.trim()));let list=await r.json();
  if(!Array.isArray(list))list=(list.results||[]).filter(c=>/^PPL/.test(c.feature_code||'')&&(c.population||0)>=1000)
   .map(c=>({id:c.id,value:[c.name,c.admin1,c.country].filter(Boolean).join(', '),cc:c.country_code}));
  res.innerHTML='';if(!list.length)res.textContent='no city found; try a larger one nearby, or coordinates';
  for(const c of list.slice(0,8)){const b=document.createElement('button');b.type='button';
   b.textContent=c.value;b.onclick=()=>{geo.value=c.id;nm.value=c.value;cc.value=c.cc||'';lat.value='';lon.value='';
   cur.textContent=c.value+' (save to apply)';res.innerHTML='';q.value='';
   if(c.cc==='IL')document.getElementsByName('israel')[0].checked=false;};res.appendChild(b)}
 }catch(e){res.textContent='search failed'}},350)});
const geo=document.getElementById('geo'),nm=document.getElementById('nm'),cc=document.getElementById('cc'),
 lat=document.getElementById('lat'),lon=document.getElementById('lon'),cur=document.getElementById('cur');
setInterval(()=>{document.getElementById('scr').src='/screen.bmp?'+Date.now()},4000);
</script>"#;

/// What the page says about the calendar, next to the location.
#[derive(Clone, Copy, Debug, Default)]
pub struct PageStatus<'a> {
    pub error: Option<&'a str>,
    pub fetching: bool,
}

/// The settings page. `notice` goes under the screenshot (result of a
/// save); `extras` is device-specific HTML placed after the form.
pub fn settings_page(out: &mut impl Write, s: &Settings, st: &PageStatus, notice: &str, extras: &str) -> fmt::Result {
    let checked = |b: bool| if b { "checked" } else { "" };
    let sel = |v: u8, cur: u8| if v == cur { "selected" } else { "" };
    let place = if s.place_name.is_empty() { "not set".to_string() } else { html_escape(&s.place_name) };
    let status = match (st.error, st.fetching) {
        (_, true) => "<span class=dim>loading calendar&hellip;</span>".to_string(),
        (Some(e), _) => format!("<span class=err>{}</span>", html_escape(e)),
        _ => String::new(),
    };

    let mut candle = String::new();
    for (v, label) in [
        (0u8, "Hebcal default (18 min; Jerusalem 40, Haifa 30)"),
        (15, "15 min"),
        (18, "18 min"),
        (20, "20 min"),
        (22, "22 min"),
        (30, "30 min"),
        (40, "40 min"),
    ] {
        candle.push_str(&format!("<option value={v} {}>{label}</option>", sel(v, s.candle_minutes)));
    }
    let mut units = String::new();
    for (v, label) in [
        (0u8, "Automatic (&deg;F in the US)"),
        (1, "&deg;C, km/h, hPa"),
        (2, "&deg;C, m/s, mmHg"),
        (3, "&deg;F, mph, inHg"),
    ] {
        units.push_str(&format!("<option value={v} {}>{label}</option>", sel(v, s.units)));
    }
    let mut havdalah = String::new();
    for (v, label) in [
        (0u8, "Nightfall, sun 8.5&deg; below horizon"),
        (42, "42 min after sunset"),
        (50, "50 min after sunset"),
        (72, "72 min after sunset"),
    ] {
        havdalah.push_str(&format!("<option value={v} {}>{label}</option>", sel(v, s.havdalah_minutes)));
    }

    write!(
        out,
        "<!doctype html><html><head><meta charset=utf-8>\
<meta name=viewport content='width=device-width,initial-scale=1'><title>JewCal</title>{CSS}</head><body>\
<h1>JEWCAL</h1>\
<img class=screen id=scr src='/screen.bmp' alt='current screen'>{notice}\
<form method=post action=/save>\
<fieldset><legend>Location</legend>\
<div>Now: <span id=cur class=cur>{place}</span> {status}</div>\
<input id=q placeholder='Search a city&hellip;' autocomplete=off>\
<div id=res></div>\
<input type=hidden name=geo id=geo value='{geo}'><input type=hidden name=name id=nm value='{name}'><input type=hidden name=cc id=cc value='{cc}'>\
<details><summary class=dim>or coordinates</summary>\
<label>Latitude <input type=text name=lat id=lat value='{lat}' placeholder='31.7683'></label>\
<label>Longitude <input type=text name=lon id=lon value='{lon}' placeholder='35.2137'></label></details>\
<label><input type=checkbox name=israel {israel}> Israel schedule outside Israel <span class=dim>(automatic for Israeli cities)</span></label>\
</fieldset>\
<fieldset><legend>Times</legend>\
<label>Candle lighting<br><select name=candle>{candle}</select></label>\
<label>Havdalah<br><select name=havdalah>{havdalah}</select></label>\
</fieldset>\
<fieldset><legend>Holidays</legend>\
<label><input type=checkbox name=minor {minor}> Minor holidays (Tu BiShvat, Lag BaOmer&hellip;)</label>\
<label><input type=checkbox name=modern {modern}> Israeli national days</label>\
</fieldset>\
<fieldset><legend>Display</legend>\
<label>Other screens every <input type=number name=cycle min=1 max=60 value={cycle}> min, \
each for <input type=number name=dwell min=3 max=60 value={dwell}> s</label>\
<label>Brightness day <input type=number name=bday min=5 max=100 value={bday}> %, \
night <input type=number name=bnight min=0 max=100 value={bnight}> %</label>\
<label>Night from <input type=number name=nfrom min=0 max=23 value={nfrom}>:00 \
to <input type=number name=nto min=0 max=23 value={nto}>:00</label>\
<label>Weather units<br><select name=units>{units}</select></label>\
</fieldset>\
<button>Save</button></form>{extras}\
<p class=dim>Calendar data: <a href='https://www.hebcal.com'>Hebcal.com</a> (CC BY 4.0). \
Weather data: <a href='https://open-meteo.com'>Open-Meteo.com</a> (CC BY 4.0). \
Times come from Hebcal for the chosen location; ask your rabbi about local customs.</p>\
{SEARCH_JS}</body></html>",
        geo = s.geonameid.map_or(String::new(), |g| g.to_string()),
        name = html_escape(&s.place_name),
        cc = html_escape(&s.cc),
        lat = html_escape(&s.lat),
        lon = html_escape(&s.lon),
        israel = checked(s.force_israel),
        minor = checked(s.minor),
        modern = checked(s.modern),
        cycle = s.cycle_min,
        dwell = s.dwell_s,
        bday = s.bright_day,
        bnight = s.bright_night,
        nfrom = s.night_from,
        nto = s.night_to,
    )
}

/// First-run setup on the access point: pick a network, enter a password.
pub fn setup_page(out: &mut impl Write, networks: &[String]) -> fmt::Result {
    let mut options = String::new();
    for n in networks {
        let n = html_escape(n);
        options.push_str(&format!("<option value=\"{n}\">{n}</option>"));
    }
    write!(
        out,
        "<!doctype html><html><head><meta charset=utf-8>\
<meta name=viewport content='width=device-width,initial-scale=1'><title>JewCal setup</title>{CSS}</head><body>\
<h1>JEWCAL SETUP</h1><form method=post action=/wifi><fieldset><legend>WiFi</legend>\
<label>Network<br><select name=pick>{options}</select></label>\
<label>or type its name<br><input type=text name=ssid autocomplete=off></label>\
<label>Password<br><input type=password name=pass autocomplete=off></label>\
</fieldset><button>Connect</button></form>\
<p class=dim>After connecting, open the address shown on the screen to choose your city.</p></body></html>"
    )
}

/// A bare page around one message (WiFi saved, errors).
pub fn message_page(out: &mut impl Write, body: &str) -> fmt::Result {
    write!(out, "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'><title>JewCal</title>{CSS}<h1>JEWCAL</h1>{body}")
}

/// The answer to a submitted WiFi form: `Ok(message)` after the
/// credentials have been accepted, `Err(message)` when they are unusable.
pub fn wifi_form(ssid: Option<String>, pass: &str) -> Result<(String, String), String> {
    match ssid {
        Some(ssid) if ssid.len() <= 32 && pass.len() <= 64 => {
            let msg = format!(
                "<p class=ok>Saved. JewCal restarts and joins <b>{}</b>. Its address appears on the screen.</p>",
                html_escape(&ssid)
            );
            Ok((ssid, msg))
        }
        _ => Err("<p class=err>Pick a network (SSID up to 32 characters).</p>".into()),
    }
}

/// The network picked or typed in the WiFi form.
pub fn wifi_ssid(field: impl Fn(&str) -> Option<String>) -> Option<String> {
    field("ssid").filter(|s| !s.is_empty()).or_else(|| field("pick"))
}

// ── Settings form ────────────────────────────────────────────────────────────

/// What a saved form changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Applied {
    /// The calendar must be fetched again (location or times changed).
    pub refetch: bool,
    pub units_changed: bool,
}

/// Apply a submitted settings form to `s`. `field` returns a decoded form
/// field (`None` when absent, e.g. an unchecked box). On error `s` is left
/// unchanged.
pub fn apply_form(s: &mut Settings, field: impl Fn(&str) -> Option<String>) -> Result<Applied, String> {
    let f = |k: &str| field(k).unwrap_or_default();
    let mut n = s.clone();
    let old_key = n.fetch_key();
    let old_units = n.units;

    let (lat, lon) = (f("lat").trim().to_string(), f("lon").trim().to_string());
    if !lat.is_empty() || !lon.is_empty() {
        check_degrees(&lat, 90).map_err(|e| format!("latitude: {e}"))?;
        check_degrees(&lon, 180).map_err(|e| format!("longitude: {e}"))?;
        if n.lat != lat || n.lon != lon {
            n.tzid.clear();
        }
        n.place_name = format!("{lat}, {lon}");
        n.geonameid = None;
        n.cc.clear();
        n.lat = lat;
        n.lon = lon;
    } else if let Ok(id) = f("geo").trim().parse::<u32>() {
        if n.geonameid != Some(id) {
            n.tzid.clear();
        }
        n.geonameid = Some(id);
        n.place_name = f("name").chars().take(60).collect();
        n.cc = f("cc").chars().take(2).collect();
        n.lat.clear();
        n.lon.clear();
    }
    n.force_israel = field("israel").is_some();
    n.minor = field("minor").is_some();
    n.modern = field("modern").is_some();
    n.candle_minutes = num(&f("candle"), 0, 90)?;
    n.havdalah_minutes = num(&f("havdalah"), 0, 120)?;
    n.cycle_min = num(&f("cycle"), 1, 60)?;
    n.dwell_s = num(&f("dwell"), 3, 60)?;
    n.bright_day = num(&f("bday"), 5, 100)?;
    n.bright_night = num(&f("bnight"), 0, 100)?;
    n.night_from = num(&f("nfrom"), 0, 23)?;
    n.night_to = num(&f("nto"), 0, 23)?;
    n.units = num(&f("units"), 0, 3)?;

    let applied = Applied { refetch: n.fetch_key() != old_key, units_changed: n.units != old_units };
    *s = n;
    Ok(applied)
}

/// The line shown under the screenshot after a save.
pub fn save_notice(result: &Result<Applied, String>) -> String {
    match result {
        Ok(a) if a.refetch => "<p class=ok>Saved. Loading the calendar for the new settings&hellip;</p>".to_string(),
        Ok(_) => "<p class=ok>Saved.</p>".to_string(),
        Err(e) => format!("<p class=err>Not saved: {}</p>", html_escape(e)),
    }
}

fn num(v: &str, min: u8, max: u8) -> Result<u8, String> {
    let n: u8 = v.trim().parse().map_err(|_| format!("{v:?} is not a number"))?;
    if !(min..=max).contains(&n) {
        return Err(format!("{n} is outside {min}..{max}"));
    }
    Ok(n)
}

/// `[-+]D[D[D]][.DDDD…]` with |integer part| ≤ `limit`, checked as text so
/// the value is sent to Hebcal exactly as typed (no floats on the device).
pub fn check_degrees(v: &str, limit: u32) -> Result<(), &'static str> {
    let unsigned = v.strip_prefix(['-', '+']).unwrap_or(v);
    let (int, frac) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if int.is_empty() || int.len() > 3 || !int.bytes().all(|b| b.is_ascii_digit()) {
        return Err("expected decimal degrees, e.g. 50.4501");
    }
    if frac.len() > 8 || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return Err("expected decimal degrees, e.g. 50.4501");
    }
    let whole: u32 = int.parse().map_err(|_| "bad number")?;
    if whole > limit || (whole == limit && frac.bytes().any(|b| b != b'0')) {
        return Err("out of range");
    }
    Ok(())
}

// ── Helpers (from Clawdmeter's main.rs) ──────────────────────────────────────

/// One field of an `application/x-www-form-urlencoded` body (or query).
pub fn form_field(body: &str, name: &str) -> Option<String> {
    body.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (k == name).then(|| url_decode(v))
    })
}

pub fn query_param(uri: &str, name: &str) -> Option<String> {
    form_field(uri.split_once('?')?.1, name)
}

pub fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            // Slice bytes, not the str: a raw multi-byte char after '%' would
            // otherwise panic on a char boundary.
            b'%' if i + 2 < bytes.len() => {
                let hex = core::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(b) => {
                        out.push(b);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// 8-bit palettized BMP header for the 120×120 canvas (16 colors used).
pub fn bmp_header() -> Vec<u8> {
    let pixels = (W * H) as u32;
    let offset = 14 + 40 + 16 * 4;
    let mut h = Vec::with_capacity(offset as usize);
    h.extend_from_slice(b"BM");
    h.extend_from_slice(&(offset + pixels).to_le_bytes());
    h.extend_from_slice(&[0; 4]);
    h.extend_from_slice(&offset.to_le_bytes());
    h.extend_from_slice(&40u32.to_le_bytes());
    h.extend_from_slice(&(W).to_le_bytes());
    h.extend_from_slice(&(H).to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes());
    h.extend_from_slice(&8u16.to_le_bytes());
    h.extend_from_slice(&0u32.to_le_bytes());
    h.extend_from_slice(&pixels.to_le_bytes());
    h.extend_from_slice(&2835u32.to_le_bytes());
    h.extend_from_slice(&2835u32.to_le_bytes());
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&16u32.to_le_bytes());
    for c in SWEETIE16 {
        h.extend_from_slice(&[c as u8, (c >> 8) as u8, (c >> 16) as u8, 0]);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;

    fn form(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: BTreeMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |k| map.get(k).cloned()
    }

    const DISPLAY: [(&str, &str); 9] = [
        ("candle", "0"),
        ("havdalah", "0"),
        ("cycle", "3"),
        ("dwell", "10"),
        ("bday", "80"),
        ("bnight", "15"),
        ("nfrom", "23"),
        ("nto", "6"),
        ("units", "0"),
    ];

    #[test]
    fn picking_a_city_refetches() {
        let mut s = Settings { tzid: "Europe/London".into(), ..Settings::default() };
        let mut f = DISPLAY.to_vec();
        f.extend([("geo", "281184"), ("name", "Jerusalem, Israel"), ("cc", "IL"), ("minor", "on"), ("modern", "on")]);
        let a = apply_form(&mut s, form(&f)).unwrap();
        assert_eq!(a, Applied { refetch: true, units_changed: false });
        assert_eq!((s.geonameid, s.cc.as_str(), s.tzid.as_str()), (Some(281184), "IL", ""));
        // Saving the same form again changes nothing Hebcal cares about.
        assert_eq!(apply_form(&mut s, form(&f)).unwrap(), Applied { refetch: false, units_changed: false });
        // Unchecked boxes are absent from a form: minor/modern turn off.
        let a = apply_form(&mut s, form(&DISPLAY)).unwrap();
        assert!(a.refetch && !s.minor && !s.modern);
    }

    #[test]
    fn bad_input_leaves_settings_alone() {
        let mut s = Settings::default();
        let mut f = DISPLAY.to_vec();
        f[2] = ("cycle", "99");
        assert_eq!(apply_form(&mut s, form(&f)), Err("99 is outside 1..60".into()));
        f[2] = ("cycle", "x");
        assert_eq!(apply_form(&mut s, form(&f)), Err("\"x\" is not a number".into()));
        let mut f = DISPLAY.to_vec();
        f.extend([("lat", "91"), ("lon", "35.2")]);
        assert_eq!(apply_form(&mut s, form(&f)), Err("latitude: out of range".into()));
        assert_eq!(s, Settings::default());
        let mut f = DISPLAY.to_vec();
        f.extend([("lat", " 31.7683 "), ("lon", "35.2137")]);
        apply_form(&mut s, form(&f)).unwrap();
        assert_eq!((s.lat.as_str(), s.place_name.as_str(), s.geonameid), ("31.7683", "31.7683, 35.2137", None));
        assert!(
            check_degrees("-180.0", 180).is_ok()
                && check_degrees("180.5", 180).is_err()
                && check_degrees("1e3", 90).is_err()
        );
    }

    #[test]
    fn page_escapes_and_fills_values() {
        let s = Settings {
            geonameid: Some(5128581),
            place_name: "<b>New York</b>".into(),
            units: 3,
            ..Settings::default()
        };
        let mut page = String::new();
        settings_page(
            &mut page,
            &s,
            &PageStatus { error: Some("timeout & more"), fetching: false },
            "<p>n</p>",
            "<p id=x>",
        )
        .unwrap();
        assert!(page.contains("value='5128581'"));
        assert!(page.contains("&lt;b&gt;New York&lt;/b&gt;") && !page.contains("<b>New York"));
        assert!(page.contains("<span class=err>timeout &amp; more</span>"));
        assert!(page.contains("<option value=3 selected>&deg;F, mph, inHg</option>"));
        assert!(page.contains("</form><p id=x><p class=dim>Calendar data"));
        assert!(page.ends_with("</script></body></html>"));
    }

    #[test]
    fn helpers() {
        assert_eq!(url_decode("Tel+Aviv%2C%20IL%e2%80%93x"), "Tel Aviv, IL\u{2013}x");
        assert_eq!(url_decode("%zz%4"), "%zz%4");
        assert_eq!(url_decode("caf%C3%A9%"), "caf\u{e9}%");
        assert_eq!(query_param("/cities?q=jeru&x=1", "q").as_deref(), Some("jeru"));
        assert_eq!(form_field("a=1&israel&b=", "israel").as_deref(), Some(""));
        assert_eq!(bmp_header().len(), 14 + 40 + 16 * 4);
        assert_eq!(wifi_ssid(form(&[("ssid", ""), ("pick", "Home")])).as_deref(), Some("Home"));
        assert!(wifi_form(Some("x".repeat(33)), "").is_err());
    }
}
