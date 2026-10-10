//! Seasons of the year: counting the Omer, and the month of Elul. (The Ten
//! Days of Return have their bar on the Yom Kippur screen and a ribbon on the
//! main screen.)

use crate::prelude::*;
use jewcal_model::classify::Kind;
use jewcal_model::snapshot::{lag_baomer, rosh_hashana_after, sefira, shavuot, Season, Snapshot};
use jewcal_model::time::Weekday;

use super::{title_bar, Ctx};
use crate::canvas::{Canvas, W};
use crate::fonts::{BODY, TINY};
use crate::palette::*;
use crate::sprites;
use crate::text::{self, fit, wrap};

fn plural(n: u8, one: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {one}S")
    }
}

/// The traditional formula: "TODAY IS 23 DAYS, WHICH ARE 3 WEEKS AND
/// 2 DAYS OF THE OMER".
fn formula(day: u8) -> String {
    let (w, d) = (day / 7, day % 7);
    if w == 0 {
        format!("TODAY IS {} OF THE OMER", plural(day, "DAY"))
    } else if d == 0 {
        format!("TODAY IS {day} DAYS, WHICH ARE {} OF THE OMER", plural(w, "WEEK"))
    } else {
        format!("TODAY IS {day} DAYS, WHICH ARE {} AND {} OF THE OMER", plural(w, "WEEK"), plural(d, "DAY"))
    }
}

pub fn draw_omer(c: &mut Canvas, s: &Snapshot, _ctx: &Ctx) {
    let Some(Season::Omer { day }) = s.season else {
        return;
    };
    c.clear(BG);
    title_bar(c, "SEFIRAT HAOMER", &format!("DAY {day} OF 49"), YELLOW);

    // 7 weeks x 7 days; Lag BaOmer (33) in fire orange.
    let (gx, gy) = (4, 15);
    for d in 1..=49u8 {
        let (col, row) = (((d - 1) % 7) as i32, ((d - 1) / 7) as i32);
        let (x, y) = (gx + col * 8, gy + row * 6);
        let color = if d == day {
            WHITE
        } else if d < day {
            if d == 33 {
                ORANGE
            } else {
                YELLOW
            }
        } else {
            DARK
        };
        c.fill_rect(x, y, 7, 5, color);
        if d == 33 && d > day {
            c.rect(x, y, 7, 5, ORANGE);
        }
    }

    let x = 66;
    c.text(&TINY, 1, x, 16, if s.hebrew.eve { "TONIGHT" } else { "TODAY" }, DIM);
    c.text_shadow(&BODY, 2, x, 23, &day.to_string(), YELLOW, BLACK);
    let (w, d) = (day / 7, day % 7);
    let mut y = 40;
    if w > 0 {
        c.text(&TINY, 1, x, y, &plural(w, "WEEK"), WHITE);
        y += 7;
    }
    if d > 0 {
        c.text(&TINY, 1, x, y, &plural(d, "DAY"), WHITE);
    }

    if let Some(sf) = sefira(day) {
        let f = fit(&TINY, 1, &sf.to_ascii_uppercase(), W - 8);
        c.text(&TINY, 1, 4, 61, &f.text, YELLOW);
    }
    for (i, l) in wrap(&TINY, 1, &formula(day), W - 8, 3).iter().enumerate() {
        c.text(&TINY, 1, 4, 70 + i as i32 * 7, l, WHITE);
    }

    let year = s.hebrew.date.year;
    let until = |date: jewcal_model::CivilDate| date.days() - s.hebrew.civil.days();
    let lag = until(lag_baomer(year));
    let lag_line = match lag {
        0 => "LAG BAOMER TODAY".to_string(),
        n if n > 0 => format!("LAG BAOMER IN {n}D \u{B7} {}", lag_baomer(year).weekday().short()),
        _ => String::new(),
    };
    if !lag_line.is_empty() {
        c.text(&TINY, 1, 4, 98, &lag_line, ORANGE);
    }
    let sh = until(shavuot(year));
    let sh_line = if sh == 1 { "SHAVUOT TOMORROW".to_string() } else { format!("SHAVUOT IN {sh}D") };
    c.text(&TINY, 1, 4, 107, &sh_line, YOM_TOV);
}

pub fn draw_elul(c: &mut Canvas, s: &Snapshot, _ctx: &Ctx) {
    let Some(Season::Elul { day }) = s.season else {
        return;
    };
    c.clear(BG);
    let year = s.hebrew.date.year;
    title_bar(c, "ELUL", &format!("{year}"), SILVER);
    c.blit(&sprites::SHOFAR, 4, 14);
    c.text(&TINY, 1, 33, 16, "MONTH OF RETURN", SILVER);
    c.text(&BODY, 1, 33, 24, "ELUL", WHITE);
    c.text(&TINY, 1, 33, 34, &format!("DAY {day} OF 29"), DIM);

    // 29 days to Rosh Hashana.
    for i in 1..=29u8 {
        let x = 2 + (i as i32 - 1) * 4;
        let color = match i.cmp(&day) {
            core::cmp::Ordering::Less => SLATE,
            core::cmp::Ordering::Equal => WHITE,
            core::cmp::Ordering::Greater => DARK,
        };
        c.fill_rect(x, 44, 3, 6, color);
    }
    c.fill_rect(2 + 29 * 4, 44, 2, 6, YOM_TOV);

    let rh = rosh_hashana_after(year);
    let n = rh.days() - s.hebrew.civil.days();
    c.text(&TINY, 1, 4, 55, "ROSH HASHANA", DIM);
    c.text_right(&TINY, 1, 116, 55, &format!("{} EVE", text::weekday_date(rh.add_days(-1))), DIM);
    let rh_line = if n == 1 { "TOMORROW".to_string() } else { format!("IN {n} DAYS") };
    c.text(&BODY, 1, 4, 62, &rh_line, YELLOW);

    // The day this screen is about is the Hebrew day (it starts at sunset).
    let shofar = if s.hebrew.civil.weekday() == Weekday::Sat {
        "SHABBAT: NO SHOFAR"
    } else if day == 29 {
        "EREV R. HASHANA: NO SHOFAR"
    } else {
        "SHOFAR EACH MORNING"
    };
    c.blit(&sprites::ICON_SHOFAR, 3, 76);
    c.text(&TINY, 1, 16, 77, &fit(&TINY, 1, shofar, W - 20).text, WHITE);

    let selichot = s.model.items.iter().find(|i| i.kind == Kind::Minor && i.title == "Leil Selichot" && i.date < rh);
    if let Some(ls) = selichot {
        let t = if ls.date >= s.today {
            format!("LEIL SELICHOT: {} {}", ls.date.weekday().short(), text::date(ls.date))
        } else {
            "SELICHOT EACH MORNING".into()
        };
        c.text(&TINY, 1, 4, 88, &fit(&TINY, 1, &t, W - 8).text, SILVER);
    }
    c.text(&TINY, 1, 4, 97, "L'DAVID (PSALM 27) DAILY", DIM);
}
