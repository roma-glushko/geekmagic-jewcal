//! The current Hebrew month as a 7-column grid: Shabbat columns, Yom Tov,
//! Chol HaMoed, fasts and Rosh Chodesh marked, today outlined.

use crate::prelude::*;
use jewcal_model::snapshot::{DayMark, Snapshot};

use super::{body_center_fit, Ctx};
use crate::canvas::{Canvas, W};
use crate::fonts::TINY;
use crate::palette::*;
use crate::text::{self, fit, title};

const CELL_W: i32 = 16;
const CELL_H: i32 = 13;
const GRID_X: i32 = 4;
const GRID_Y: i32 = 21;

pub fn draw(c: &mut Canvas, s: &Snapshot, _ctx: &Ctx) {
    c.clear(BG);
    let m = &s.month;
    c.fill_rect(0, 0, W, 11, PANEL);
    body_center_fit(c, W / 2, 2, &format!("{} {}", m.name.to_ascii_uppercase(), m.year), 116, YELLOW);
    for (i, d) in ["S", "M", "T", "W", "T", "F", "S"].iter().enumerate() {
        let color = if i == 6 { SHABBAT } else { DIM };
        c.text_center(&TINY, 1, GRID_X + i as i32 * CELL_W + CELL_W / 2, 14, d, color);
    }
    let offset = m.first.weekday().index() as i32;
    for day in 1..=m.len as i32 {
        let pos = offset + day - 1;
        let (col, row) = (pos % 7, pos / 7);
        let x = GRID_X + col * CELL_W;
        let y = GRID_Y + row * CELL_H;
        let mark = m.marks[(day - 1) as usize];
        let (fill, ink) = match mark {
            DayMark::YomTov => (Some(YOM_TOV), BLACK),
            DayMark::Fast => (Some(FAST), WHITE),
            DayMark::CholHaMoed => (Some(CHOL_HAMOED), BLACK),
            DayMark::RoshChodesh => (Some(SLATE), WHITE),
            _ if col == 6 => (None, SHABBAT),
            _ => (None, SILVER),
        };
        if let Some(f) = fill {
            c.fill_rect(x + 1, y + 1, CELL_W - 2, CELL_H - 2, f);
        }
        match mark {
            DayMark::Modern => c.fill_rect(x + CELL_W - 4, y + 2, 2, 2, MODERN),
            DayMark::Holiday => c.fill_rect(x + CELL_W - 4, y + 2, 2, 2, HOLIDAY),
            _ => {}
        }
        c.text_center(&TINY, 1, x + CELL_W / 2, y + 4, &day.to_string(), ink);
        if day as u8 == m.today {
            c.rect(x, y, CELL_W, CELL_H, WHITE);
        }
    }
    if let Some(rc) = s.next_rosh_chodesh {
        let f = fit(&TINY, 1, &title(&rc.title), W - 6);
        c.text(&TINY, 1, 3, 104, &f.text, ROSH_CHODESH);
        let when = format!(
            "{} {} \u{B7} {}",
            text::weekdays(rc.first, rc.last),
            text::range(rc.first, rc.last),
            text::days_until(s.today, rc.first, false)
        );
        c.text(&TINY, 1, 3, 112, &when, DIM);
    }
}
