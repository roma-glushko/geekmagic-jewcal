//! The main screen, shown most of the time:
//!
//! ```text
//! ┌──────────────────────────┐  sky band follows the real sun (day/dusk/night)
//! │ 18:42        FRI 9 OCT   │  clock, Gregorian date
//! │              28 TISHREI  │  Hebrew date (moon glyph after sunset)
//! ├──────────────────────────┤
//! │ [art]  SHABBAT    3 DAYS │  hero: what is happening now / next
//! │        BERESHIT          │
//! │ CANDLES      STARTS IN   │
//! │ FRI 18:03    5H 22M      │
//! │ * note                   │
//! ├──────────────────────────┤
//! │ NEXT               IN 2D │  the next holiday after the hero
//! │ ☾ ROSH CHODESH CHESHVAN  │
//! └──────────────────────────┘
//! ```

use crate::prelude::*;
use jewcal_model::calendar::{EveningKind, Fast, Note, Period, PeriodDay};
use jewcal_model::snapshot::{Sky, Snapshot, State};
use jewcal_model::time::{month_short, CivilDate, Stamp};

use super::{art_for, draw_art, period_art, span_color, span_icon, Ctx};
use crate::canvas::{Canvas, W};
use crate::fonts::{BODY, TINY};
use crate::palette::*;
use crate::sprite::Sprite;
use crate::sprites;
use crate::text::{self, countdown, fit, title, wrap};

const HEADER_H: i32 = 24;
const PANEL_Y: i32 = 26;
const PANEL_H: i32 = 59;
/// The season ribbon (Elul, Ten Days of Return, Omer) along the bottom.
const RIBBON_Y: i32 = 112;
const TEXT_X: i32 = 33;
const TEXT_W: i32 = 82;
const LOW_Y: i32 = PANEL_Y + 32;
const RIGHT_X: i32 = 66;

enum Hero<'a> {
    InPeriod(&'a Period, usize),
    NextPeriod(&'a Period),
    InFast(&'a Fast),
    FastSoon(&'a Fast),
    Nothing,
}

fn pick<'a>(s: &Snapshot<'a>) -> Hero<'a> {
    match s.state {
        State::InPeriod { period, day } => Hero::InPeriod(period, day),
        State::InFast(f) => Hero::InFast(f),
        State::Normal => {
            let fast_first = match (s.fast_soon, s.next_period) {
                (Some(f), Some(p)) => f.begins.is_none_or(|b| b.utc < p.evening_utc(0)),
                (Some(_), None) => true,
                _ => false,
            };
            match (fast_first, s.fast_soon, s.next_period) {
                (true, Some(f), _) => Hero::FastSoon(f),
                (_, _, Some(p)) => Hero::NextPeriod(p),
                _ => Hero::Nothing,
            }
        }
    }
}

pub fn draw(c: &mut Canvas, s: &Snapshot, ctx: &Ctx) {
    c.clear(BG);
    header(c, s, ctx);
    let hero = pick(s);
    // The dates the hero card already covers; the footer shows what comes
    // next outside them (which can be sooner, e.g. a fast before Shabbat).
    let covered = match &hero {
        Hero::InPeriod(p, _) | Hero::NextPeriod(p) => Some((p.first_date(), p.last_date())),
        Hero::InFast(f) | Hero::FastSoon(f) => Some((f.date, f.date)),
        Hero::Nothing => None,
    };
    match hero {
        Hero::InPeriod(p, day) => in_period(c, s, p, day, ctx.frame),
        Hero::NextPeriod(p) => next_period(c, s, p, ctx.frame),
        Hero::InFast(f) => fast(c, s, f, true),
        Hero::FastSoon(f) => fast(c, s, f, false),
        Hero::Nothing => {
            c.panel(2, PANEL_Y, 116, PANEL_H, MUTED, PANEL);
            c.text_center(&BODY, 1, W / 2, PANEL_Y + 26, "NO DATA YET", DIM);
        }
    }
    footer(c, s, covered);
    ribbon(c, s);
}

/// `ELUL 18 · ROSH HASHANA IN 14D`, `TEN DAYS OF RETURN · DAY 5`,
/// `OMER DAY 23 · 3W 2D`, or after sunset `TONIGHT: OMER 24 · 3W 3D`.
fn ribbon(c: &mut Canvas, s: &Snapshot) {
    use jewcal_model::snapshot::{rosh_hashana_after, Season};
    let Some(season) = s.season else { return };
    let (t, bg, ink) = match season {
        Season::Elul { day } => {
            let rh = rosh_hashana_after(s.hebrew.date.year);
            let n = rh.days() - s.hebrew.civil.days();
            (format!("ELUL {day} \u{B7} ROSH HASHANA IN {n}D"), PURPLE, WHITE)
        }
        Season::TenDays { day } => (format!("TEN DAYS OF RETURN \u{B7} DAY {day}"), SLATE, WHITE),
        Season::ThreeWeeks { day } => {
            use jewcal_model::snapshot::NINE_DAYS_FROM;
            let to_9av = 22 - day as i64;
            let tail = if to_9av == 0 { "TISHA B'AV".to_string() } else { format!("9 AV IN {to_9av}D") };
            let name = if day >= NINE_DAYS_FROM { "THE NINE DAYS" } else { "THE THREE WEEKS" };
            (format!("{name} \u{B7} {tail}"), DARK, SILVER)
        }
        Season::Omer { day } => {
            let wd = match (day / 7, day % 7) {
                (0, d) => format!("{d}D"),
                (w, 0) => format!("{w}W"),
                (w, d) => format!("{w}W {d}D"),
            };
            let lead = if s.hebrew.eve { format!("TONIGHT: OMER {day}") } else { format!("OMER DAY {day}") };
            (format!("{lead} \u{B7} {wd}"), YELLOW, BLACK)
        }
    };
    c.fill_rect(0, RIBBON_Y, W, 120 - RIBBON_Y, bg);
    let f = fit(&TINY, 1, &t, W - 4);
    c.text_center(&TINY, 1, W / 2, RIBBON_Y + 2, &f.text, ink);
}

// ── Header ───────────────────────────────────────────────────────────────────

const STARS: [(i32, i32); 9] = [(58, 3), (66, 15), (52, 19), (62, 8), (40, 2), (47, 21), (108, 21), (36, 20), (71, 2)];

fn header(c: &mut Canvas, s: &Snapshot, ctx: &Ctx) {
    let frame = ctx.frame;
    let stops: &[u8] = match s.sky {
        Sky::Day => &[BLUE, SKY],
        Sky::Dusk => &[PURPLE, RED, ORANGE],
        Sky::Night => &[BLACK, NAVY],
    };
    c.gradient(0, 0, W, HEADER_H, stops);
    if s.sky == Sky::Night {
        for (i, (x, y)) in STARS.iter().enumerate() {
            let twinkle = (frame / 3 + i as u32 * 5).is_multiple_of(11);
            c.pset(*x, *y, if twinkle { SLATE } else { SILVER });
        }
    }
    let clock = format!("{:02}:{:02}", s.local.h, s.local.m);
    c.text_shadow(&BODY, 2, 4, 5, &clock, WHITE, BLACK);

    let greg = format!("{} {} {}", s.today.weekday().short(), s.today.d, month_short(s.today.m));
    right_shadow(c, 117, 3, &greg, WHITE);
    let heb = format!("{} {}", s.hebrew.date.day, s.hebrew.month_name().to_ascii_uppercase());
    let hw = TINY.width(&heb, 1);
    right_shadow(c, 117, 10, &heb, YELLOW);
    if s.hebrew.eve {
        // The Hebrew day began at sunset.
        c.blit(&sprites::ICON_MOON, 117 - hw - 9, 9);
    }
    // Third line: the weather when we have it, else the Hebrew year.
    match ctx.weather.filter(|w| !w.is_stale(s.now)) {
        Some(w) => {
            let t = format!("{}\u{B0}{}", jewcal_model::weather::deg(w.current.temp), w.units.temp());
            let tw = TINY.width(&t, 1);
            right_shadow(c, 117, 17, &t, WHITE);
            super::weather::glyph(c, w, 117 - tw - 12, 16);
        }
        None => right_shadow(c, 117, 17, &s.hebrew.date.year.to_string(), SILVER),
    }
    c.hline(0, HEADER_H, W, DARK);
}

fn right_shadow(c: &mut Canvas, right: i32, y: i32, t: &str, color: u8) {
    let w = TINY.width(t, 1);
    c.text_shadow(&TINY, 1, right - w, y, t, color, BLACK);
}

// ── Hero ─────────────────────────────────────────────────────────────────────

fn panel(c: &mut Canvas, color: u8) {
    c.panel(2, PANEL_Y, 116, PANEL_H, color, PANEL);
    // Dotted divider between the title zone and the times.
    for x in (6..114).step_by(2) {
        c.pset(x, LOW_Y - 2, MUTED);
    }
}

/// Label (TINY, accent color) and up to two title lines (BODY).
fn heading(c: &mut Canvas, label: &str, color: u8, lines: &[String]) {
    c.text(&TINY, 1, TEXT_X, PANEL_Y + 4, label, color);
    for (i, l) in lines.iter().take(2).enumerate() {
        c.text(&BODY, 1, TEXT_X, PANEL_Y + 11 + i as i32 * 9, l, WHITE);
    }
}

/// Accent pill at the top right; `short` is used when `t` would run into
/// the label on its left.
fn badge(c: &mut Canvas, label: &str, t: &str, short: &str, color: u8) {
    let label_end = TEXT_X + TINY.width(label, 1) + 6;
    let t = if 114 - (TINY.width(t, 1) + 4) < label_end { short } else { t };
    let w = TINY.width(t, 1) + 4;
    let x = 114 - w;
    c.fill_rect(x, PANEL_Y + 3, w, 7, color);
    c.text(&TINY, 1, x + 2, PANEL_Y + 4, t, BLACK);
}

/// Lower-left time block: icon + label, then `WED 19:23`.
fn time_block(c: &mut Canvas, icon: &Sprite, label: &str, when: &str, color: u8) {
    c.blit(icon, 6, LOW_Y);
    c.text(&TINY, 1, 15, LOW_Y + 2, label, color);
    c.text(&BODY, 1, 6, LOW_Y + 10, when, WHITE);
}

/// Lower-right countdown block.
fn countdown_block(c: &mut Canvas, label: &str, secs: Option<i64>) {
    c.text(&TINY, 1, RIGHT_X, LOW_Y + 2, label, DIM);
    let t = secs.map_or_else(|| "--".into(), countdown);
    c.text(&BODY, 1, RIGHT_X, LOW_Y + 10, &t, YELLOW);
}

fn note(c: &mut Canvas, t: &str, color: u8) {
    let f = fit(&TINY, 1, t, 108);
    c.text(&TINY, 1, 6, LOW_Y + 20, &f.text, color);
}

fn when(s: Option<Stamp>) -> String {
    s.map_or_else(|| "--:--".into(), |s| format!("{} {}", s.local_date().weekday().short(), text::hm(s)))
}

/// `PESACH I` → `PESACH`, keeping names like `ROSH HASHANA II` → `ROSH HASHANA`.
fn base_name(t: &str) -> String {
    let d = title(t);
    match d.rsplit_once(' ') {
        Some((head, tail)) if ["I", "II", "VII", "VIII"].contains(&tail) => head.to_string(),
        _ => d,
    }
}

fn period_titles(p: &Period) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for d in &p.days {
        if let Some(yt) = &d.yom_tov {
            let n = base_name(yt);
            if !names.contains(&n) {
                names.push(n);
            }
        }
    }
    if names.is_empty() {
        let parsha = p.days.iter().find_map(|d| d.parsha.as_deref()).map(title);
        return wrap(&BODY, 1, &parsha.unwrap_or_else(|| "SHABBAT".into()), TEXT_W, 2);
    }
    let mut lines = wrap(&BODY, 1, &names.join(" & "), TEXT_W, 2);
    if p.has_shabbat() && lines.len() < 2 {
        lines.push("+ SHABBAT".into());
    }
    lines
}

fn day_title(d: &PeriodDay) -> Vec<String> {
    if let Some(yt) = &d.yom_tov {
        let mut v = wrap(&BODY, 1, &title(yt), TEXT_W, 2);
        if d.shabbat && v.len() < 2 {
            v.push("SHABBAT".into());
        }
        v
    } else if d.chol_hamoed.is_some() {
        vec!["SHABBAT".into(), "CHOL HAMOED".into()]
    } else if let Some(p) = &d.parsha {
        wrap(&BODY, 1, &title(p), TEXT_W, 2)
    } else {
        vec!["SHABBAT".into()]
    }
}

fn next_period(c: &mut Canvas, s: &Snapshot, p: &Period, frame: u32) {
    let (art, flames, color) = period_art(p);
    panel(c, color);
    draw_art(c, art, flames, 6, PANEL_Y + 4, frame);
    let label = if p.yom_kippur {
        "YOM KIPPUR"
    } else if p.has_yom_tov() {
        "YOM TOV"
    } else {
        "SHABBAT"
    };
    heading(c, label, color, &period_titles(p));
    if p.days.len() > 1 {
        let n = p.days.len();
        badge(c, label, &format!("{n} DAYS"), &format!("{n}D"), color);
    }
    let begins = p.begins();
    time_block(c, &sprites::ICON_CANDLE, "CANDLES", &when(begins), color);
    countdown_block(c, "STARTS IN", begins.map(|b| b.utc - s.now));

    let erev = p.evenings[0].date;
    let n = p.notes.iter().find_map(|n| match n {
        Note::EruvTavshilin(d) if *d == erev => Some(format!("ERUV TAVSHILIN {}", erev_word(s.today, *d))),
        Note::ChanukahFirst(d) if *d == erev => Some("LIGHT CHANUKAH CANDLES FIRST".into()),
        _ => None,
    });
    let specials = p.days.iter().flat_map(|d| d.specials.iter()).map(|t| title(t)).next();
    if let Some(n) = n.or(specials) {
        note(c, &n, DIM);
    }
}

fn erev_word(today: CivilDate, d: CivilDate) -> String {
    if d == today {
        "TODAY".into()
    } else {
        d.weekday().short().into()
    }
}

fn in_period(c: &mut Canvas, s: &Snapshot, p: &Period, day: usize, frame: u32) {
    let d = &p.days[day];
    let (art, flames, color) = match &d.yom_tov {
        Some(yt) => {
            let (a, f) = art_for(yt);
            (a, f, YOM_TOV)
        }
        None => {
            let (a, f) = art_for("Shabbat");
            (a, f, SHABBAT)
        }
    };
    panel(c, color);
    draw_art(c, art, flames, 6, PANEL_Y + 4, frame);
    let greeting = if p.yom_kippur {
        "G'MAR TOV"
    } else if d.yom_tov.is_some() {
        "CHAG SAMEACH"
    } else {
        "SHABBAT SHALOM"
    };
    heading(c, greeting, color, &day_title(d));
    if p.days.len() > 1 {
        let (k, n) = (day + 1, p.days.len());
        badge(c, greeting, &format!("DAY {k}/{n}"), &format!("{k}/{n}"), color);
    }
    let Some(i) = p.next_evening(s.now) else {
        return;
    };
    let e = &p.evenings[i];
    let (icon, label): (&Sprite, &str) = match e.how {
        EveningKind::Havdalah => (&sprites::ICON_HAVDALAH, "HAVDALAH"),
        EveningKind::CandlesAfterNightfall => (&sprites::ICON_NIGHT, "CANDLES*"),
        EveningKind::CandlesBeforeSunset => (&sprites::ICON_CANDLE, "CANDLES"),
    };
    time_block(c, icon, label, &when(e.at), color);
    countdown_block(c, "IN", Some(p.evening_utc(i) - s.now));

    let fast_after = p.notes.iter().find_map(|n| match n {
        Note::FastFollows { name, begins } if e.how == EveningKind::Havdalah => {
            Some(format!("{} BEGINS {}", title(name), text::hm(*begins)))
        }
        _ => None,
    });
    let msg = match e.how {
        EveningKind::CandlesAfterNightfall if e.date.weekday() == jewcal_model::Weekday::Sat => {
            Some("* AFTER SHABBAT, HAVDALAH IN KIDDUSH".to_string())
        }
        EveningKind::CandlesAfterNightfall => Some("* AFTER NIGHTFALL, FROM A FLAME".to_string()),
        _ => fast_after,
    };
    if let Some(m) = msg {
        note(c, &m, DIM);
    }
}

fn fast(c: &mut Canvas, s: &Snapshot, f: &Fast, active: bool) {
    panel(c, FAST);
    c.blit(&sprites::PLATE, 6, PANEL_Y + 4);
    heading(c, if active { "FAST DAY" } else { "FAST" }, FAST, &wrap(&BODY, 1, &title(&f.name), TEXT_W, 2));
    if active {
        let friday = f.date.weekday() == jewcal_model::Weekday::Fri;
        let end = match f.ends {
            Some(e) => when(Some(e)),
            None if friday => "AT SHABBAT".into(),
            None => "--:--".into(),
        };
        time_block(c, &sprites::ICON_PLATE, "ENDS", &end, FAST);
        countdown_block(c, "ENDS IN", f.ends.map(|e| e.utc - s.now));
        if let Some(b) = f.begins {
            note(c, &format!("BEGAN {}", when(Some(b))), DIM);
        }
    } else {
        let begin = f.begins.map_or_else(|| "AT DAWN".into(), |b| when(Some(b)));
        time_block(c, &sprites::ICON_PLATE, "BEGINS", &begin, FAST);
        countdown_block(c, "STARTS IN", f.begins.map(|b| b.utc - s.now));
        if let Some(e) = f.ends {
            note(c, &format!("ENDS {}", when(Some(e))), DIM);
        }
    }
}

// ── Footer ───────────────────────────────────────────────────────────────────

fn footer(c: &mut Canvas, s: &Snapshot, covered: Option<(CivilDate, CivilDate)>) {
    let Some(sp) = s
        .upcoming
        .iter()
        .copied()
        .find(|sp| sp.first > s.today && covered.is_none_or(|(a, b)| sp.last < a || sp.first > b))
        .or(s.next_yom_tov)
    else {
        return;
    };
    let y = PANEL_Y + PANEL_H + 3; // leaves the bottom rows for the season ribbon
    let color = span_color(sp.kind);
    c.text(&TINY, 1, 4, y, "NEXT", DIM);
    let until = text::days_until(s.today, sp.first, sp.from_evening);
    c.text_right(&TINY, 1, 116, y, &until, color);
    c.blit(span_icon(sp.kind), 3, y + 8);
    let t = fit(&BODY, 1, &title(&sp.title), 104);
    c.text(&BODY, 1, 13, y + 8, &t.text, WHITE);
    let dates = format!("{} {}", text::weekdays(sp.first, sp.last), text::range(sp.first, sp.last));
    c.text(&TINY, 1, 13, y + 18, &dates, DIM);
}
