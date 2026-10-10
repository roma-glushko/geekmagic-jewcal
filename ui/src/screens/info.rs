//! Device screens: info (long-press), first-run setup, and status messages.

use crate::prelude::*;
use jewcal_model::snapshot::Snapshot;

use super::{title_bar, Ctx, Status};
use crate::canvas::{Canvas, W};
use crate::fonts::{BODY, TINY};
use crate::palette::*;
use crate::sprites;
use crate::text::{fit, wrap};

pub fn draw_info(c: &mut Canvas, _s: Option<&Snapshot>, ctx: &Ctx) {
    c.clear(BG);
    title_bar(c, "JEWCAL", "SETTINGS", SKY);
    let st: &Status = ctx.status;
    // Where to open the settings page: the mDNS name, and the IP as fallback.
    c.text(&TINY, 1, 3, 15, "OPEN IN A BROWSER", DIM);
    match (&st.host, &st.url) {
        (Some(host), Some(url)) => {
            let f = fit(&BODY, 1, &host.to_ascii_uppercase(), W - 6);
            c.text(&BODY, 1, 3, 23, &f.text, SKY);
            let ip = url.trim_start_matches("http://").trim_end_matches('/');
            c.text(&TINY, 1, 3, 33, &format!("OR {ip}"), SKY);
        }
        (None, Some(url)) => {
            let ip = url.trim_start_matches("http://").trim_end_matches('/');
            c.text(&BODY, 1, 3, 23, &fit(&BODY, 1, ip, W - 6).text, SKY);
        }
        _ => {
            c.text(&BODY, 1, 3, 23, "NO NETWORK", MUTED);
        }
    }
    let mut y = 43;
    let mut row = |c: &mut Canvas, label: &str, value: &str, color: u8| {
        c.text(&TINY, 1, 3, y, label, DIM);
        let f = fit(&BODY, 1, value, W - 6);
        c.text(&BODY, 1, 3, y + 7, &f.text, color);
        y += 18;
    };
    let city = st.location.as_deref().map(|l| l.split(',').next().unwrap_or(l).trim().to_string());
    row(c, "LOCATION", city.as_deref().unwrap_or("NOT SET"), WHITE);
    row(c, "SCHEDULE", if st.israel { "ISRAEL" } else { "DIASPORA" }, WHITE);
    row(c, "UPDATED", st.updated.as_deref().unwrap_or("NEVER"), WHITE);
    if let Some(e) = &st.error {
        for (i, l) in wrap(&TINY, 1, &e.to_ascii_uppercase(), W - 6, 1).iter().enumerate() {
            c.text(&TINY, 1, 3, 98 + i as i32 * 7, l, RED);
        }
    }
    c.text_center(&TINY, 1, W / 2, 112, "HEBCAL.COM \u{B7} OPEN-METEO.COM", MUTED);
}

/// First run: join the device's own WiFi network and open its page.
pub fn draw_setup(c: &mut Canvas, ssid: &str, url: &str, frame: u32) {
    c.clear(BG);
    title_bar(c, "JEWCAL", "SETUP", SKY);
    let bob = if (frame / 4).is_multiple_of(2) { 0 } else { 1 };
    c.blit(&sprites::ICON_WIFI, W / 2 - 4, 18 + bob);
    c.text_center(&TINY, 1, W / 2, 34, "1. JOIN WIFI", DIM);
    let f = fit(&BODY, 1, ssid, W - 6);
    c.text_center(&BODY, 1, W / 2, 42, &f.text, YELLOW);
    c.text_center(&TINY, 1, W / 2, 60, "2. OPEN", DIM);
    let f = fit(&BODY, 1, url, W - 6);
    c.text_center(&BODY, 1, W / 2, 68, &f.text, SKY);
    c.text_center(&TINY, 1, W / 2, 86, "3. PICK YOUR NETWORK", DIM);
    c.text_center(&TINY, 1, W / 2, 93, "AND PASSWORD", DIM);
}

/// Boot / waiting / error messages: a candle, a title, a few lines.
pub fn draw_message(c: &mut Canvas, title: &str, lines: &[&str], color: u8, frame: u32) {
    c.clear(BG);
    let f = (frame / 2) as usize % sprites::FLAME.len();
    c.blit(&sprites::FLAME[f], W / 2 - 2, 22);
    c.fill_rect(W / 2 - 1, 29, 3, 14, WHITE);
    c.vline(W / 2 + 1, 29, 14, SILVER);
    c.fill_rect(W / 2 - 4, 43, 9, 2, YELLOW);
    let t = fit(&BODY, 1, title, W - 6);
    c.text_center(&BODY, 1, W / 2, 54, &t.text, color);
    let mut y = 66;
    for l in lines {
        for part in wrap(&TINY, 1, &l.to_ascii_uppercase(), W - 6, 3) {
            c.text_center(&TINY, 1, W / 2, y, &part, DIM);
            y += 7;
        }
    }
}
