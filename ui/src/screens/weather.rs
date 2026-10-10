//! Weather: an animated pixel-art sky, temperature, wind, humidity,
//! pressure and a 3-day forecast. Data: Open-Meteo.com.

use crate::prelude::*;
use jewcal_model::snapshot::Snapshot;
use jewcal_model::weather::{deg, Condition, Weather};
use jewcal_model::Weekday;

use super::{body_center_fit, title_bar, Ctx};
use crate::canvas::{Canvas, W};
use crate::fonts::{BODY, TINY};
use crate::palette::*;
use crate::sprite::Sprite;
use crate::sprites;
use crate::text::fit;

pub fn draw(c: &mut Canvas, s: &Snapshot, ctx: &Ctx) {
    c.clear(BG);
    let place = ctx
        .status
        .location
        .as_deref()
        .map(|l| l.split(',').next().unwrap_or(l).trim().to_ascii_uppercase())
        .unwrap_or_default();
    let place = fit(&TINY, 1, &place, 70).text;
    title_bar(c, "WEATHER", &place, SKY);
    let Some(w) = ctx.weather.filter(|w| !w.is_stale(s.now)) else {
        c.text_center(&BODY, 1, W / 2, 56, "NO WEATHER YET", DIM);
        return;
    };
    let cur = &w.current;
    art(c, cur.condition, cur.is_day, 2, 13, ctx.frame);

    let t = format!("{}\u{B0}", deg(cur.temp));
    c.text_shadow(&BODY, 2, 48, 14, &t, WHITE, BLACK);
    c.text(&TINY, 1, 48 + BODY.width(&t, 2) + 1, 15, w.units.temp(), DIM);
    if let Some(f) = cur.feels {
        c.text(&TINY, 1, 48, 31, &format!("FEELS {}\u{B0}", deg(f)), DIM);
    }
    if let Some(d) = w.days.iter().find(|d| d.date == s.today) {
        if let (Some(hi), Some(lo)) = (d.max, d.min) {
            c.text(&TINY, 1, 48, 38, &format!("H {}\u{B0}  L {}\u{B0}", deg(hi), deg(lo)), DIM);
        }
    }
    body_center_fit(c, W / 2, 47, cur.condition.label(), 116, YELLOW);

    let wind = cur.wind.map(|v| {
        let dir = cur.wind_dir.map_or("", jewcal_model::weather::compass);
        (deg(v).to_string(), format!("{} {dir}", w.units.wind()))
    });
    stat(c, 4, 58, &sprites::ICON_WIND, wind);
    stat(c, 64, 58, &sprites::ICON_HUMIDITY, cur.humidity.map(|h| (h.to_string(), "%".into())));
    stat(
        c,
        4,
        70,
        &sprites::ICON_PRESSURE,
        cur.pressure.map(|p| {
            let (n, u) = w.units.pressure(p);
            (n, u.to_string())
        }),
    );
    let rain_today = w.days.iter().find(|d| d.date == s.today).and_then(|d| d.precip);
    stat(c, 64, 70, &sprites::ICON_UMBRELLA, rain_today.map(|p| (p.to_string(), "%".into())));

    // Three days.
    c.hline(2, 81, 116, DARK);
    for (i, d) in w.days.iter().filter(|d| d.date >= s.today).take(3).enumerate() {
        let cx = 21 + i as i32 * 39;
        let label = if d.date == s.today { "TODAY" } else { d.date.weekday().short() };
        c.text_center(&TINY, 1, cx, 84, label, if d.date.weekday() == Weekday::Sat { SHABBAT } else { DIM });
        mini(c, d.condition, cx - 7, 91);
        if let (Some(hi), Some(lo)) = (d.max, d.min) {
            c.text_center(&TINY, 1, cx, 104, &format!("{}/{}", deg(hi), deg(lo)), WHITE);
        }
        if let Some(p) = d.precip.filter(|p| *p >= 10) {
            c.text_center(&TINY, 1, cx, 111, &format!("{p}%"), SKY);
        }
    }
}

/// Icon, number (BODY) and unit (TINY).
fn stat(c: &mut Canvas, x: i32, y: i32, icon: &Sprite, value: Option<(String, String)>) {
    c.blit(icon, x, y);
    match value {
        Some((n, unit)) => {
            let w = c.text(&BODY, 1, x + 10, y, &n, WHITE);
            c.text(&TINY, 1, x + 12 + w, y + 2, &unit, DIM);
        }
        None => {
            c.text(&BODY, 1, x + 10, y, "--", MUTED);
        }
    }
}

const RAYS: [(i32, i32); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

fn sun(c: &mut Canvas, x: i32, y: i32, frame: u32) {
    let (cx, cy) = (x + 5, y + 5);
    let len = 2 + ((frame / 4) % 2) as i32;
    for (dx, dy) in RAYS {
        let r0 = if dx == 0 || dy == 0 { 7 } else { 6 };
        for k in 0..len {
            c.pset(cx + dx * (r0 + k), cy + dy * (r0 + k), if k == 0 { ORANGE } else { YELLOW });
        }
    }
    c.blit(&sprites::SUN, x, y);
}

fn night(c: &mut Canvas, x: i32, y: i32, frame: u32) {
    c.blit(&sprites::MOON_NIGHT, x, y);
    for (i, (sx, sy)) in [(16, 1), (20, 8), (13, 13), (24, 15)].iter().enumerate() {
        let on = !(frame / 3 + i as u32 * 4).is_multiple_of(9);
        c.pset(x + sx, y + sy, if on { WHITE } else { SLATE });
    }
}

fn cloud(c: &mut Canvas, x: i32, y: i32, dark: bool) {
    if dark {
        c.blit_mapped(&sprites::CLOUD, x, y, &[(WHITE, SLATE), (SILVER, DARK)]);
    } else {
        c.blit(&sprites::CLOUD, x, y);
    }
}

/// Fixed scatter so drops and flakes do not line up into streaks.
const SCATTER_X: [i32; 8] = [1, 13, 7, 18, 4, 10, 16, 2];
const SCATTER_Y: [i32; 8] = [0, 9, 4, 12, 7, 2, 11, 5];

fn drops(c: &mut Canvas, x: i32, y: i32, frame: u32, n: i32, len: i32, freezing: bool) {
    for i in 0..n.min(8) {
        let dx = SCATTER_X[i as usize];
        let dy = (SCATTER_Y[i as usize] + frame as i32 * 2) % 14;
        let color = if freezing && i % 2 == 0 { WHITE } else { SKY };
        for k in 0..len {
            c.pset(x + dx, y + dy + k, color);
        }
    }
}

fn flakes(c: &mut Canvas, x: i32, y: i32, frame: u32, n: i32) {
    for i in 0..n.min(8) {
        let drift = ((frame as i32 / 3 + i) % 3) - 1;
        let fx = x + SCATTER_X[i as usize] + drift;
        let fy = y + (SCATTER_Y[i as usize] + frame as i32) % 14;
        c.pset(fx, fy, WHITE);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            c.pset(fx + dx, fy + dy, SILVER);
        }
    }
}

fn fog(c: &mut Canvas, x: i32, y: i32, frame: u32) {
    for row in 0..3 {
        let off = ((frame / 3) as i32 + row * 2) % 4;
        for xx in 0..34 {
            if (xx + off) % 4 != 0 {
                c.pset(x + xx, y + row * 4, if row == 1 { SLATE } else { SILVER });
            }
        }
    }
}

const BOLT: [(i32, i32); 10] = [(3, 0), (2, 1), (2, 2), (1, 3), (2, 3), (3, 3), (4, 3), (3, 4), (3, 5), (2, 6)];

fn bolt(c: &mut Canvas, x: i32, y: i32, frame: u32) {
    if (frame / 2) % 6 < 4 {
        for (dx, dy) in BOLT {
            c.pset(x + dx, y + dy, YELLOW);
        }
    }
}

/// The big animated sky for the weather screen (~44×30 art pixels).
pub(crate) fn art(c: &mut Canvas, cond: Condition, day: bool, x: i32, y: i32, frame: u32) {
    let body = |c: &mut Canvas, bx: i32, by: i32| {
        if day {
            sun(c, bx, by, frame)
        } else {
            night(c, bx, by, frame)
        }
    };
    match cond {
        Condition::Clear => body(c, x + 13, y + 7),
        Condition::MainlyClear => {
            body(c, x + 9, y + 4);
            c.blit(&sprites::CLOUD_SMALL, x + 26, y + 20);
        }
        Condition::PartlyCloudy => {
            body(c, x + 5, y + 2);
            cloud(c, x + 16, y + 13, false);
        }
        Condition::Overcast => {
            cloud(c, x + 2, y + 5, true);
            cloud(c, x + 16, y + 13, false);
        }
        Condition::Fog => {
            cloud(c, x + 9, y + 2, false);
            fog(c, x + 4, y + 16, frame);
        }
        Condition::Drizzle => {
            cloud(c, x + 9, y + 2, false);
            drops(c, x + 11, y + 13, frame, 5, 1, false);
        }
        Condition::Rain => {
            cloud(c, x + 9, y + 2, true);
            drops(c, x + 11, y + 13, frame, 8, 2, false);
        }
        Condition::FreezingRain => {
            cloud(c, x + 9, y + 2, true);
            drops(c, x + 11, y + 13, frame, 8, 2, true);
        }
        Condition::Showers => {
            body(c, x + 3, y);
            cloud(c, x + 13, y + 7, false);
            drops(c, x + 15, y + 17, frame, 6, 2, false);
        }
        Condition::Snow => {
            cloud(c, x + 9, y + 2, false);
            flakes(c, x + 11, y + 14, frame, 6);
        }
        Condition::SnowShowers => {
            body(c, x + 3, y);
            cloud(c, x + 13, y + 7, false);
            flakes(c, x + 15, y + 18, frame, 5);
        }
        Condition::Thunder => {
            cloud(c, x + 9, y + 2, true);
            bolt(c, x + 18, y + 13, frame);
            drops(c, x + 10, y + 13, frame, 4, 2, false);
        }
    }
}

/// 14×12 forecast icon (daytime).
pub(crate) fn mini(c: &mut Canvas, cond: Condition, x: i32, y: i32) {
    let dark = [(WHITE, SLATE), (SILVER, DARK)];
    match cond {
        Condition::Clear => c.blit(&sprites::SUN_SMALL, x + 3, y + 2),
        Condition::MainlyClear | Condition::PartlyCloudy => {
            c.blit(&sprites::SUN_SMALL, x, y);
            c.blit(&sprites::CLOUD_SMALL, x + 4, y + 5);
        }
        Condition::Overcast => {
            c.blit_mapped(&sprites::CLOUD_SMALL, x, y + 1, &dark);
            c.blit(&sprites::CLOUD_SMALL, x + 4, y + 5);
        }
        Condition::Fog => {
            c.blit(&sprites::CLOUD_SMALL, x + 2, y);
            c.hline(x + 1, y + 8, 12, SILVER);
            c.hline(x + 3, y + 10, 10, SLATE);
        }
        Condition::Snow | Condition::SnowShowers => {
            c.blit(&sprites::CLOUD_SMALL, x + 2, y);
            for (dx, dy) in [(3, 8), (7, 10), (11, 8)] {
                c.pset(x + dx, y + dy, WHITE);
            }
        }
        Condition::Thunder => {
            c.blit_mapped(&sprites::CLOUD_SMALL, x + 2, y, &dark);
            for (dx, dy) in [(7, 6), (6, 7), (6, 8), (7, 8), (8, 8), (7, 9), (6, 10)] {
                c.pset(x + dx, y + dy, YELLOW);
            }
        }
        _ => {
            c.blit(&sprites::CLOUD_SMALL, x + 2, y);
            for dx in [3, 7, 11] {
                c.vline(x + dx, y + 7 + (dx % 2), 2, SKY);
            }
        }
    }
}

/// Up to 10×7: the condition next to the temperature in the main header.
pub(crate) fn glyph(c: &mut Canvas, w: &Weather, x: i32, y: i32) {
    match (w.current.condition, w.current.is_day) {
        (Condition::Clear | Condition::MainlyClear, true) => c.blit(&sprites::SUN_SMALL, x + 2, y),
        (Condition::Clear | Condition::MainlyClear, false) => c.blit(&sprites::MOON_SMALL, x + 3, y),
        (cond, _) => {
            c.blit(&sprites::CLOUD_SMALL, x, y);
            match cond {
                Condition::Drizzle | Condition::Rain | Condition::Showers | Condition::FreezingRain => {
                    c.pset(x + 3, y + 6, SKY);
                    c.pset(x + 7, y + 6, SKY);
                }
                Condition::Snow | Condition::SnowShowers => {
                    c.pset(x + 3, y + 6, WHITE);
                    c.pset(x + 7, y + 6, WHITE);
                }
                Condition::Thunder => c.pset(x + 5, y + 6, YELLOW),
                _ => {}
            }
        }
    }
}
