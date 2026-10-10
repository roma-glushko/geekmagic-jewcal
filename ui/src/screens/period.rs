//! The evening-by-evening timeline of the current or next Shabbat/Yom Tov:
//! where stacked days (two days of Yom Tov running into Shabbat) are spelled
//! out.
//!
//! ```text
//! NEXT · YOM TOV              APR 22-24
//!        PESACH + SHABBAT
//!   WED      THU      FRI      SAT
//!   🕯       ★🕯      🕯        ⚱
//!  19:23    20:26    19:25    20:28
//!     ▓▓▓▓▓▓▓▓░░░░░░░░▒▒▒▒▒▒▒▒
//!     PES I   PES II   SHAB
//! ERUV TAVSHILIN WED
//! ★ AFTER NIGHTFALL, FROM A FLAME
//! ```

use crate::prelude::*;
use jewcal_model::calendar::{EveningKind, Note, Period, PeriodDay};
use jewcal_model::snapshot::{Snapshot, State};

use super::{body_center_fit, period_art, title_bar, Ctx};
use crate::canvas::{Canvas, W};
use crate::fonts::{BODY, NUM, TINY};
use crate::palette::*;
use crate::sprite::Sprite;
use crate::sprites;
use crate::text::{self, fit, title};

pub fn draw(c: &mut Canvas, s: &Snapshot, _ctx: &Ctx) {
    c.clear(BG);
    let Some(p) = s.focus_period() else {
        title_bar(c, "SHABBAT & CHAG", "", SHABBAT);
        c.text_center(&BODY, 1, W / 2, 56, "NO DATA YET", DIM);
        return;
    };
    let (_, _, color) = period_art(p);
    let now_in = matches!(s.state, State::InPeriod { period, .. } if core::ptr::eq(period, p));
    let kind = if p.yom_kippur {
        "YOM KIPPUR"
    } else if p.has_yom_tov() {
        "YOM TOV"
    } else {
        "SHABBAT"
    };
    let label = format!("{} \u{B7} {kind}", if now_in { "NOW" } else { "NEXT" });
    title_bar(c, &label, &text::range(p.first_date(), p.last_date()), color);
    body_center_fit(c, W / 2, 15, &headline(p), 116, WHITE);

    // ── Evenings ─────────────────────────────────────────────────────────────
    let n = p.evenings.len() as i32;
    let col = 116 / n;
    let cx = |i: i32| 2 + col * i + col / 2;
    let next = p.next_evening(s.now);
    for (i, e) in p.evenings.iter().enumerate() {
        let x = cx(i as i32);
        let highlight = next == Some(i);
        if highlight {
            c.panel(x - col / 2 + 1, 25, col - 2, 32, color, PANEL);
        }
        c.text_center(&TINY, 1, x, 28, e.date.weekday().short(), if highlight { color } else { DIM });
        let icon: &Sprite = match e.how {
            EveningKind::CandlesBeforeSunset => &sprites::ICON_CANDLE,
            EveningKind::CandlesAfterNightfall => &sprites::ICON_NIGHT,
            EveningKind::Havdalah => &sprites::ICON_HAVDALAH,
        };
        c.blit(icon, x - 3, 35);
        c.text_center(&NUM, 1, x, 46, &text::opt_hm(e.at), WHITE);
    }

    // ── Days between the evenings ────────────────────────────────────────────
    for (k, d) in p.days.iter().enumerate() {
        let (x0, x1) = (cx(k as i32), cx(k as i32 + 1));
        let y = 60;
        match (d.yom_tov.is_some(), d.shabbat) {
            (true, true) => c.dither_rect(x0, y, x1 - x0, 5, YOM_TOV, SHABBAT, 8),
            (true, false) => c.fill_rect(x0, y, x1 - x0, 5, YOM_TOV),
            _ => c.fill_rect(x0, y, x1 - x0, 5, SHABBAT),
        }
        c.vline(x0, y - 2, 9, WHITE);
        c.vline(x1, y - 2, 9, WHITE);
        let lbl = fit(&TINY, 1, &short_day(d), x1 - x0 - 2);
        c.text_center(&TINY, 1, (x0 + x1) / 2, 68, &lbl.text, if d.yom_tov.is_some() { YOM_TOV } else { SHABBAT });
    }

    // ── Notes ────────────────────────────────────────────────────────────────
    let mut lines: Vec<(String, u8)> = Vec::new();
    for note in &p.notes {
        match note {
            Note::EruvTavshilin(d) => {
                lines.push((format!("ERUV TAVSHILIN {} {}", d.weekday().short(), text::date(*d)), YOM_TOV))
            }
            Note::HavdalahInKiddush(_) => lines.push(("SAT: HAVDALAH IN KIDDUSH".into(), DIM)),
            Note::ChanukahFirst(_) => lines.push(("LIGHT CHANUKAH BEFORE SHABBAT".into(), ORANGE)),
            Note::FastFollows { name, begins } => {
                lines.push((format!("{} BEGINS {}", title(name), text::hm(*begins)), FAST))
            }
        }
    }
    if p.evenings.iter().any(|e| e.how == EveningKind::CandlesAfterNightfall) {
        lines.push(("STAR = LIGHT AFTER NIGHTFALL".into(), DIM));
        lines.push(("FROM AN EXISTING FLAME".into(), DIM));
    }
    if !p.has_yom_tov() {
        if let Some(d) = p.days.iter().find(|d| d.shabbat) {
            if let Some(ps) = &d.parsha {
                c.text_center(&TINY, 1, W / 2, 78, "PARASHAT", DIM);
                body_center_fit(c, W / 2, 85, &title(ps), 116, WHITE);
            }
            for sp in &d.specials {
                lines.push((title(sp), SILVER));
            }
        }
    }
    let y0 = if p.has_yom_tov() { 78 } else { 96 };
    for (i, (t, color)) in lines.iter().take(((118 - y0) / 7) as usize).enumerate() {
        let f = fit(&TINY, 1, t, 114);
        c.text(&TINY, 1, 3, y0 + i as i32 * 7, &f.text, *color);
    }
}

/// `PESACH + SHABBAT`, `ROSH HASHANA`, `SHABBAT BERESHIT`.
fn headline(p: &Period) -> String {
    let mut names: Vec<String> = Vec::new();
    for d in &p.days {
        if let Some(yt) = &d.yom_tov {
            let t = title(yt);
            let base = match t.rsplit_once(' ') {
                Some((head, tail)) if ["I", "II", "VII", "VIII"].contains(&tail) => head.to_string(),
                _ => t,
            };
            if !names.contains(&base) {
                names.push(base);
            }
        }
    }
    if names.is_empty() {
        // The parsha gets its own line below the timeline.
        return "SHABBAT".into();
    }
    let mut s = names.join(" & ");
    if p.has_shabbat() && !p.days.iter().all(|d| d.yom_tov.is_some()) {
        s.push_str(" + SHABBAT");
    }
    s
}

/// Short label for one day of the strip: `PESACH II` → `PES II`.
fn short_day(d: &PeriodDay) -> String {
    match &d.yom_tov {
        Some(yt) => {
            let t = title(yt);
            if t == "ROSH HASHANA" {
                "RH I".into()
            } else if t == "ROSH HASHANA II" {
                "RH II".into()
            } else if t == "YOM KIPPUR" {
                "YK".into()
            } else if t.starts_with("SHMINI") {
                "SH. ATZ".into()
            } else if t.starts_with("SIMCHAT") {
                "S. TORAH".into()
            } else {
                t.replace("PESACH", "PES").replace("SUKKOT", "SUK").replace("SHAVUOT", "SHAV")
            }
        }
        None if d.chol_hamoed.is_some() => "SHAB".into(),
        None => "SHABBAT".into(),
    }
}
