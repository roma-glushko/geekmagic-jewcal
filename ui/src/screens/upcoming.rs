//! The next holidays, fasts and Rosh Chodesh, one per row.

use crate::prelude::*;
use jewcal_model::snapshot::Snapshot;

use super::{span_color, span_icon, title_bar, Ctx};
use crate::canvas::{Canvas, W};
use crate::fonts::{BODY, TINY};
use crate::palette::*;
use crate::text::{self, fit, title};

const ROWS: usize = 5;
const ROW_H: i32 = 19;

pub fn draw(c: &mut Canvas, s: &Snapshot, _ctx: &Ctx) {
    c.clear(BG);
    title_bar(c, "UPCOMING", &text::weekday_date(s.today), YOM_TOV);
    for (i, sp) in s.upcoming.iter().take(ROWS).enumerate() {
        let y = 14 + i as i32 * ROW_H;
        let color = span_color(sp.kind);
        c.vline(1, y, ROW_H - 3, color);
        c.blit(span_icon(sp.kind), 4, y);
        let until = text::days_until(s.today, sp.first, sp.from_evening);
        let until_w = TINY.width(&until, 1);
        let t = fit(&BODY, 1, &title(&sp.title), W - 14 - 2);
        c.text(&BODY, 1, 13, y, &t.text, WHITE);
        let dates = format!("{} {}", text::weekdays(sp.first, sp.last), text::range(sp.first, sp.last));
        let d = fit(&TINY, 1, &dates, W - 16 - until_w - 4);
        c.text(&TINY, 1, 13, y + 9, &d.text, DIM);
        c.text_right(&TINY, 1, W - 3, y + 9, &until, color);
    }
    // The next Yom Tov, when it is further out than the rows above.
    if let Some(yt) = s.next_yom_tov {
        if !s.upcoming.iter().take(ROWS).any(|sp| core::ptr::eq(*sp, yt)) {
            let name = title(&yt.title);
            let base =
                name.split(' ').filter(|w| !w.chars().all(|ch| "IV-".contains(ch))).collect::<Vec<_>>().join(" ");
            let t = format!("NEXT CHAG: {base} {}", text::days_until(s.today, yt.first, false));
            let f = fit(&TINY, 1, &t, W - 6);
            c.text(&TINY, 1, 3, 112, &f.text, YOM_TOV);
        }
    }
}
