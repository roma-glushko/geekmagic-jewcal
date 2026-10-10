//! The Chanukah screen: a menorah with tonight's candles.

use crate::prelude::*;
use jewcal_model::snapshot::Snapshot;
use jewcal_model::time::Weekday;

use super::{title_bar, Ctx};
use crate::canvas::{Canvas, W};
use crate::fonts::{BODY, TINY};
use crate::palette::*;
use crate::sprites;
use crate::text;

const BRANCH_X: [i32; 9] = [12, 24, 36, 48, 60, 72, 84, 96, 108];
const SHAMASH: usize = 4;

pub fn draw_chanukah(c: &mut Canvas, s: &Snapshot, ctx: &Ctx) {
    let Some(ch) = s.chanukah else { return };
    c.clear(BG);
    c.gradient(0, 11, W, 80, &[BLACK, NAVY]);
    title_bar(c, "CHANUKAH", &text::range(ch.first_evening, ch.last_day), ORANGE);

    // Menorah: base, stem, bar, cups.
    c.fill_rect(44, 86, 32, 3, YELLOW);
    c.fill_rect(52, 83, 16, 3, ORANGE);
    c.fill_rect(59, 62, 3, 21, YELLOW);
    c.fill_rect(10, 60, 101, 3, YELLOW);
    c.hline(10, 63, 101, ORANGE);
    for (i, &x) in BRANCH_X.iter().enumerate() {
        let top = if i == SHAMASH { 38 } else { 46 };
        c.fill_rect(x - 2, 56, 5, 4, YELLOW);
        c.fill_rect(x - 1, top, 3, 56 - top, if i == SHAMASH { SILVER } else { WHITE });
        c.vline(x + 1, top, 56 - top, SILVER);
    }
    // Candles go in right to left; on night N the N rightmost (plus the
    // shamash) are lit.
    let n = ch.tonight as usize;
    if n > 0 {
        let candles: Vec<usize> = (0..9).filter(|&i| i != SHAMASH).collect();
        let lit = candles.iter().rev().take(n).chain(core::iter::once(&SHAMASH));
        for (k, &i) in lit.enumerate() {
            let x = BRANCH_X[i];
            let top = if i == SHAMASH { 38 } else { 46 };
            let f = ((ctx.frame / 2) as usize + k) % sprites::FLAME.len();
            c.blit(&sprites::FLAME[f], x - 2, top - 6);
        }
    }

    let (big, small) = if n > 0 {
        let at = ch.at.map_or_else(|| "AFTER SUNSET".into(), |t| format!("LIGHT {}", text::hm(t)));
        (format!("NIGHT {n}"), at)
    } else if s.today < ch.first_evening {
        (format!("BEGINS {}", text::days_until(s.today, ch.first_evening, true)), text::weekday_date(ch.first_evening))
    } else {
        ("8TH DAY".into(), "CHAG SAMEACH".into())
    };
    c.text_center(&BODY, 1, W / 2, 95, &big, YELLOW);
    c.text_center(&TINY, 1, W / 2, 105, &small, WHITE);
    if n > 0 && s.today.weekday() == Weekday::Fri {
        c.text_center(&TINY, 1, W / 2, 112, "BEFORE SHABBAT CANDLES", ORANGE);
    }
}
