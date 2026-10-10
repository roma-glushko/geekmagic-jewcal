//! A screen of its own for each major holiday, shown in the rotation from a
//! week before until it ends. Each shows only what matters for that day:
//! the new year and the shofar on Rosh Hashana, the fast on Yom Kippur,
//! lulav and the day strip on Sukkot, the Omer on Shavuot, the four
//! mitzvot on Purim.

use crate::prelude::*;
use jewcal_model::calendar::Period;
use jewcal_model::hebrew::{HDate, TISHREI};
use jewcal_model::snapshot::{FestDay, FestDayKind, Holiday, HolidayView, Season, Snapshot, NINE_DAYS_FROM};
use jewcal_model::time::{CivilDate, Stamp, Weekday};

use super::{draw_art, title_bar, Ctx};
use crate::canvas::{Canvas, W};
use crate::fonts::{BODY, TINY};
use crate::palette::*;
use crate::sprite::Sprite;
use crate::sprites;
use crate::text::{self, countdown, fit, title};

const TEXT_X: i32 = 33;

/// The holiday day it is now, by the Hebrew day (which starts at sunset;
/// once candles are lit the first day counts as begun).
fn jewish_day(s: &Snapshot, h: &HolidayView) -> CivilDate {
    let d = s.hebrew.civil;
    match h.period {
        Some(p) if p.contains(s.now) && d < h.first => h.first,
        _ => d,
    }
}

/// "BEGINS IN 5H 22M", "DAY 2 OF 2", "TODAY", "IN 6D".
fn status(s: &Snapshot, h: &HolidayView) -> String {
    let days = (h.last.days() - h.first.days() + 1) as usize;
    let day = Some(jewish_day(s, h));
    if let Some(d) = day.filter(|d| *d >= h.first && *d <= h.last) {
        let i = (d.days() - h.first.days()) as usize + 1;
        return if days > 1 { format!("DAY {i} OF {days}") } else { "TODAY".into() };
    }
    match h.period.and_then(Period::begins) {
        Some(b) if b.utc > s.now && b.utc - s.now < 2 * 86_400 => {
            format!("BEGINS IN {}", countdown(b.utc - s.now))
        }
        _ => text::days_until(s.today, h.first, false),
    }
}

/// Art on the left, then greeting, name and status.
#[allow(clippy::too_many_arguments)]
fn header(
    c: &mut Canvas,
    art: &Sprite,
    flames: &[(i32, i32)],
    frame: u32,
    x: i32,
    greeting: &str,
    name: &str,
    line: &str,
    color: u8,
) {
    draw_art(c, art, flames, 4, 14, frame);
    c.text(&TINY, 1, x, 16, greeting, color);
    c.text(&BODY, 1, x, 24, name, WHITE);
    c.text(&TINY, 1, x, 34, line, DIM);
}

fn when(s: Option<Stamp>) -> String {
    s.map_or_else(|| "--:--".into(), |s| format!("{} {}", s.local_date().weekday().short(), text::hm(s)))
}

fn line(c: &mut Canvas, y: i32, t: &str, color: u8) {
    let f = fit(&TINY, 1, t, W - 6);
    c.text(&TINY, 1, 3, y, &f.text, color);
}

// ── Rosh Hashana ─────────────────────────────────────────────────────────────

pub fn draw_rosh_hashana(c: &mut Canvas, s: &Snapshot, _ctx: &Ctx) {
    let Some(h) = s.holiday(Holiday::RoshHashana) else {
        return;
    };
    c.clear(BG);
    title_bar(c, "ROSH HASHANA", &text::range(h.first, h.last), YOM_TOV);
    c.blit(&sprites::SHOFAR, 4, 14);
    c.text(&TINY, 1, TEXT_X, 16, "SHANA TOVA", YOM_TOV);
    let year = HDate::from_civil(h.first).year.to_string();
    c.text_shadow(&BODY, 2, TEXT_X, 24, &year, YOM_TOV, BLACK);
    c.blit(&sprites::HONEY, 104, 14);
    c.text(&TINY, 1, TEXT_X, 41, &status(s, h), DIM);

    // One card per day of the period: is the shofar blown?
    let days: Vec<(CivilDate, bool, Option<&str>)> = match h.period {
        Some(p) => p.days.iter().map(|d| (d.date, d.shabbat, d.yom_tov.as_deref())).collect(),
        None => vec![(h.first, false, Some("Rosh Hashana")), (h.last, false, Some("Rosh Hashana II"))],
    };
    let n = days.len() as i32;
    let w = 116 / n;
    for (i, (date, shabbat, yt)) in days.iter().enumerate() {
        let x = 2 + i as i32 * w;
        let cx = x + w / 2;
        let is_rh = yt.is_some_and(|t| t.starts_with("Rosh Hashana"));
        let border = if *date == jewish_day(s, h) {
            WHITE
        } else if is_rh {
            YOM_TOV
        } else {
            SHABBAT
        };
        c.panel(x + 1, 49, w - 2, 40, border, PANEL);
        c.text_center(&TINY, 1, cx, 52, &format!("{} {}", date.weekday().short(), date.d), DIM);
        let label = match yt {
            Some(t) if t == &"Rosh Hashana II" => "DAY 2",
            Some(t) if t.starts_with("Rosh Hashana") => "DAY 1",
            _ => "SHABBAT",
        };
        c.text_center(&TINY, 1, cx, 60, label, if is_rh { YOM_TOV } else { SHABBAT });
        if is_rh && !shabbat {
            c.blit(&sprites::ICON_SHOFAR, cx - 5, 69);
            c.text_center(&TINY, 1, cx, 80, "SHOFAR", YOM_TOV);
        } else if is_rh {
            c.blit(&sprites::ICON_CANDLE, cx - 3, 68);
            c.text_center(&TINY, 1, cx, 80, "NO SHOFAR", RED);
        } else {
            c.blit(&sprites::ICON_CANDLE, cx - 3, 68);
            c.text_center(&TINY, 1, cx, 80, "SHUVA", SHABBAT);
        }
    }
    if let Some(b) = h.period.and_then(Period::begins) {
        line(c, 94, &format!("CANDLES {}", when(Some(b))), WHITE);
    }
    if let Some(f) = h.fast {
        let span = match (f.begins, f.ends) {
            (Some(b), Some(e)) => format!("{}-{}", text::hm(b), text::hm(e)),
            _ => text::date(f.date),
        };
        line(c, 103, &format!("TZOM GEDALIAH {} {span}", f.date.weekday().short()), FAST);
    }
    if days.iter().any(|(_, shabbat, yt)| *shabbat && yt.is_some()) {
        line(c, 112, "NO SHOFAR ON SHABBAT", DIM);
    }
}

// ── Yom Kippur ───────────────────────────────────────────────────────────────

pub fn draw_yom_kippur(c: &mut Canvas, s: &Snapshot, ctx: &Ctx) {
    let Some(h) = s.holiday(Holiday::YomKippur) else {
        return;
    };
    c.clear(BG);
    title_bar(c, "YOM KIPPUR", &text::weekday_date(h.first), WHITE);
    header(
        c,
        &sprites::PRAYING_HANDS,
        &[],
        ctx.frame,
        TEXT_X,
        "G'MAR CHATIMA TOVA",
        "YOM KIPPUR",
        &status(s, h),
        SILVER,
    );

    // The Ten Days of Return: 1 Tishrei (Rosh Hashana) .. 10 Tishrei.
    let hd = s.hebrew.date;
    let day = (hd.month == TISHREI && hd.day <= 10).then_some(hd.day as i32);
    c.text(&TINY, 1, 4, 44, "TEN DAYS OF RETURN", DIM);
    if let Some(d) = day {
        c.text_right(&TINY, 1, 116, 44, &format!("DAY {d}"), WHITE);
    }
    for i in 0..10 {
        let x = 4 + i * 11;
        let color = match day {
            Some(d) if i + 1 == d => WHITE,
            Some(d) if i + 1 < d => SLATE,
            _ if i == 9 => SILVER,
            _ => DARK,
        };
        c.fill_rect(x, 51, 10, 6, color);
    }

    let p = h.period;
    let begins = p.and_then(Period::begins);
    let ends = p.and_then(Period::ends);
    c.blit(&sprites::ICON_CANDLE, 4, 62);
    c.text(&TINY, 1, 13, 64, "CANDLES", DIM);
    c.text_right(&BODY, 1, 116, 63, &when(begins), WHITE);
    c.blit(&sprites::ICON_HAVDALAH, 4, 74);
    c.text(&TINY, 1, 13, 76, "FAST ENDS", DIM);
    c.text_right(&BODY, 1, 116, 75, &when(ends), WHITE);
    if let (Some(b), Some(e)) = (begins, ends) {
        let mins = (e.utc - b.utc) / 60;
        line(c, 88, &format!("FAST OF ABOUT {}H {:02}M", mins / 60, mins % 60), DIM);
    }
    let big = match (begins, ends) {
        (Some(b), _) if s.now < b.utc => Some(("BEGINS IN", b.utc - s.now)),
        (_, Some(e)) if s.now < e.utc => Some(("ENDS IN", e.utc - s.now)),
        _ => None,
    };
    if let Some((label, secs)) = big {
        c.text(&TINY, 1, 4, 101, label, DIM);
        c.text(&BODY, 1, 4, 108, &countdown(secs), YELLOW);
    }
    if p.is_some_and(|p| p.has_shabbat()) {
        c.text_right(&TINY, 1, 116, 108, "ON SHABBAT", SHABBAT);
    }
}

// ── Sukkot and Pesach ────────────────────────────────────────────────────────

/// Day cells for Sukkot/Pesach: gold Yom Tov, green Chol HaMoed, a sky
/// stripe on Shabbat, today outlined.
pub(crate) fn day_strip(c: &mut Canvas, strip: &[FestDay], today: CivilDate, y: i32) {
    let n = strip.len().max(1) as i32;
    let cell = 116 / n;
    let x0 = (W - cell * n) / 2;
    for (i, d) in strip.iter().enumerate() {
        let x = x0 + i as i32 * cell;
        let (fill, ink) = match d.kind {
            FestDayKind::YomTov => (YOM_TOV, BLACK),
            FestDayKind::CholHaMoed => (CHOL_HAMOED, BLACK),
        };
        c.fill_rect(x + 1, y, cell - 2, 20, fill);
        if d.date.weekday() == Weekday::Sat {
            c.fill_rect(x + 1, y, cell - 2, 3, SHABBAT);
        }
        let lbl = fit(&TINY, 1, &day_label(d.title), cell - 2);
        c.text_center(&TINY, 1, x + cell / 2, y + 8, &lbl.text, ink);
        c.text_center(&TINY, 1, x + cell / 2, y + 23, &d.date.weekday().short()[..1], DIM);
        if d.date == today {
            c.rect(x, y - 1, cell, 22, WHITE);
        }
    }
}

/// `Pesach III (CH''M)` → `3`, Hoshana Raba → `HR`, Shmini Atzeret → `SA`.
fn day_label(t: &str) -> String {
    if t.contains("Hoshana Raba") {
        return "HR".into();
    }
    if t.starts_with("Shmini Atzeret") {
        return "SA".into();
    }
    if t.starts_with("Simchat Torah") {
        return "ST".into();
    }
    const ROMAN: [&str; 8] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII"];
    let t = t.replace(" (CH''M)", "");
    let tail = t.rsplit_once(' ').map_or("I", |(_, r)| r);
    ROMAN.iter().position(|r| *r == tail).map_or_else(|| tail.to_string(), |i| (i + 1).to_string())
}

pub fn draw_sukkot(c: &mut Canvas, s: &Snapshot, _ctx: &Ctx) {
    let Some(h) = s.holiday(Holiday::Sukkot) else {
        return;
    };
    c.clear(BG);
    title_bar(c, "SUKKOT", &text::range(h.first, h.last), CHOL_HAMOED);
    c.blit(&sprites::SUKKAH, 4, 14);
    c.blit(&sprites::LULAV, 28, 14);
    c.text(&TINY, 1, 46, 16, "CHAG SAMEACH", CHOL_HAMOED);
    c.text(&BODY, 1, 46, 24, "SUKKOT", WHITE);
    c.text(&TINY, 1, 46, 34, &status(s, h), DIM);
    let day = jewish_day(s, h);
    day_strip(c, &h.strip, day, 48);

    let today = h.strip.iter().find(|d| d.date == day);
    let note = match today {
        Some(d) if d.title.contains("Hoshana Raba") => "HOSHANA RABA: LULAV + WILLOWS".to_string(),
        Some(d) if d.title.starts_with("Sukkot") && day.weekday() == Weekday::Sat => "SHABBAT: SUKKAH, NO LULAV".into(),
        Some(d) if d.title.starts_with("Sukkot") => "TODAY: SUKKAH + LULAV".into(),
        Some(d) => title(d.title),
        None => format!("BEGINS {} EVE", text::weekday_date(h.first.add_days(-1))),
    };
    line(c, 84, &note, YELLOW);
    let named = |pred: &dyn Fn(&str) -> bool| h.strip.iter().find(|d| pred(d.title)).map(|d| d.date);
    if let Some(d) = named(&|t| t.contains("Hoshana Raba")) {
        line(c, 94, &format!("HOSHANA RABA {}", text::weekday_date(d)), DIM);
    }
    let sa = named(&|t| t.starts_with("Shmini Atzeret"));
    let st = named(&|t| t.starts_with("Simchat Torah"));
    match (sa, st) {
        (Some(a), Some(b)) if a != b => {
            line(c, 103, &format!("SHMINI ATZERET {}", text::weekday_date(a)), DIM);
            line(c, 112, &format!("SIMCHAT TORAH {}", text::weekday_date(b)), DIM);
        }
        (Some(a), _) => line(c, 103, &format!("SH. ATZERET+SIMCHAT TORAH {}", a.weekday().short()), DIM),
        _ => {}
    }
}

pub fn draw_pesach(c: &mut Canvas, s: &Snapshot, _ctx: &Ctx) {
    let Some(h) = s.holiday(Holiday::Pesach) else {
        return;
    };
    c.clear(BG);
    title_bar(c, "PESACH", &text::range(h.first, h.last), YOM_TOV);
    c.blit(&sprites::MATZAH, 4, 14);
    c.text(&TINY, 1, TEXT_X, 16, "CHAG SAMEACH", YOM_TOV);
    c.text(&BODY, 1, TEXT_X, 24, "PESACH", WHITE);
    c.text(&TINY, 1, TEXT_X, 34, &status(s, h), DIM);
    let day = jewish_day(s, h);
    day_strip(c, &h.strip, day, 48);
    let ly = 84;
    c.fill_rect(3, ly, 5, 5, YOM_TOV);
    c.text(&TINY, 1, 10, ly, "YOM TOV", DIM);
    c.fill_rect(44, ly, 5, 5, CHOL_HAMOED);
    c.text(&TINY, 1, 51, ly, "CHOL HAMOED", DIM);
    let note = match (h.strip.iter().find(|d| d.date == day), s.omer) {
        (Some(d), Some(n)) => format!("{} \u{B7} OMER {n}", title(d.title)),
        (Some(d), None) => title(d.title),
        (None, _) => format!("SEDER {} NIGHT", text::weekday_date(h.first.add_days(-1))),
    };
    line(c, 100, &note, YELLOW);
}

// ── Shavuot ──────────────────────────────────────────────────────────────────

pub fn draw_shavuot(c: &mut Canvas, s: &Snapshot, _ctx: &Ctx) {
    let Some(h) = s.holiday(Holiday::Shavuot) else {
        return;
    };
    c.clear(BG);
    title_bar(c, "SHAVUOT", &text::range(h.first, h.last), YOM_TOV);
    c.blit(&sprites::TABLETS, 4, 14);
    c.blit(&sprites::WHEAT, 28, 17);
    c.text(&TINY, 1, 44, 16, "CHAG SAMEACH", YOM_TOV);
    c.text(&BODY, 1, 44, 24, "SHAVUOT", WHITE);
    c.text(&TINY, 1, 44, 34, &status(s, h), DIM);

    // The Omer: 7 weeks × 7 days leading up to Shavuot.
    let counted = s.omer.unwrap_or(if s.hebrew.civil >= h.first { 49 } else { 0 });
    let (gx, gy) = (6, 46);
    for d in 1..=49u8 {
        let (col, row) = (((d - 1) % 7) as i32, ((d - 1) / 7) as i32);
        let color = if d == counted && counted < 49 {
            WHITE
        } else if d <= counted {
            YELLOW
        } else {
            DARK
        };
        c.fill_rect(gx + col * 8, gy + row * 6, 7, 5, color);
    }
    let x = 66;
    c.text(&TINY, 1, x, 47, "COUNTING", DIM);
    c.text(&TINY, 1, x, 53, "THE OMER", DIM);
    if counted > 0 && counted < 49 || s.omer == Some(49) {
        c.text(&BODY, 1, x, 61, &format!("DAY {counted}"), WHITE);
        let (w, d) = (counted / 7, counted % 7);
        let weeks = match w {
            0 => String::new(),
            1 => "1 WEEK".into(),
            _ => format!("{w} WEEKS"),
        };
        c.text(&TINY, 1, x, 71, &weeks, YELLOW);
        if d > 0 {
            c.text(&TINY, 1, x, 77, &format!("{d} DAY{}", if d > 1 { "S" } else { "" }), YELLOW);
        }
        if counted < 49 {
            c.text(&TINY, 1, x, 84, &format!("{} TO GO", 49 - counted), DIM);
        }
    } else {
        c.text(&BODY, 1, x, 61, "49 DAYS", YELLOW);
        c.text(&TINY, 1, x, 71, "COUNTED", DIM);
    }
    if let Some(b) = h.period.and_then(Period::begins) {
        line(c, 96, &format!("CANDLES {}", when(Some(b))), WHITE);
    }
    if h.last > h.first {
        line(c, 105, "2ND NIGHT: AFTER NIGHTFALL", DIM);
    }
}

// ── Purim ────────────────────────────────────────────────────────────────────

pub fn draw_purim(c: &mut Canvas, s: &Snapshot, _ctx: &Ctx) {
    let Some(h) = s.holiday(Holiday::Purim) else {
        return;
    };
    c.clear(BG);
    title_bar(c, "PURIM", &text::weekday_date(h.first), HOLIDAY);
    c.blit(&sprites::MASK, 4, 12);
    c.blit(&sprites::HAMANTASCH, 30, 24);
    c.text(&TINY, 1, 48, 16, "PURIM SAMEACH", HOLIDAY);
    c.text(&BODY, 1, 48, 24, "PURIM", WHITE);
    let st = if s.today == h.first { "TODAY".into() } else { status(s, h) };
    c.text(&TINY, 1, 48, 34, &st, DIM);

    let mitzvot: [(&Sprite, &str); 4] = [
        (&sprites::ICON_SCROLL, "MEGILLAH"),
        (&sprites::ICON_GIFT, "GIFTS"),
        (&sprites::ICON_COINS, "CHARITY"),
        (&sprites::ICON_FEAST, "FEAST"),
    ];
    for (i, (icon, label)) in mitzvot.iter().enumerate() {
        let x = 4 + (i as i32 % 2) * 58;
        let y = 46 + (i as i32 / 2) * 14;
        c.blit(icon, x, y);
        c.text(&TINY, 1, x + 14, y + 2, label, WHITE);
    }
    if let Some(f) = h.fast {
        let times = match (f.begins, f.ends) {
            (Some(b), Some(e)) => format!(" {}-{}", text::hm(b), text::hm(e)),
            _ => String::new(),
        };
        line(c, 78, &format!("TA'ANIT ESTHER {}{times}", f.date.weekday().short()), FAST);
    }
    if h.last > h.first {
        line(c, 88, &format!("SHUSHAN PURIM {}", text::weekday_date(h.last)), DIM);
        line(c, 95, "IN JERUSALEM", DIM);
    }
    line(c, 106, &format!("MEGILLAH {} NIGHT + DAY", h.first.add_days(-1).weekday().short()), YELLOW);
}

// ── Tisha B'Av ───────────────────────────────────────────────────────────────

pub fn draw_tisha_bav(c: &mut Canvas, s: &Snapshot, ctx: &Ctx) {
    let Some(h) = s.holiday(Holiday::TishaBav) else {
        return;
    };
    let Some(f) = h.fast else { return };
    c.clear(BG);
    title_bar(c, "TISHA B'AV", &text::weekday_date(h.first), SILVER);
    c.blit(&sprites::WALL, 2, 13);
    // A single low candle: Eicha is read by its light.
    let flame = (ctx.frame / 2) as usize % sprites::FLAME.len();
    c.blit(&sprites::FLAME[flame], 32, 25);
    c.fill_rect(33, 31, 3, 4, WHITE);
    c.vline(35, 31, 4, SILVER);
    c.text(&TINY, 1, 40, 16, "THE FAST OF AV", SILVER);
    c.text(&BODY, 1, 40, 24, "TISHA B'AV", WHITE);
    c.text(&TINY, 1, 40, 34, &status(s, h), DIM);

    // The Three Weeks: 17 Tammuz .. 9 Av, the last nine are the Nine Days.
    let day = match s.season {
        Some(Season::ThreeWeeks { day }) => Some(day),
        _ => None,
    };
    let label = match day {
        Some(d) if d >= NINE_DAYS_FROM => {
            format!("THE NINE DAYS \u{B7} DAY {}", d - NINE_DAYS_FROM + 1)
        }
        Some(d) => format!("THE THREE WEEKS \u{B7} DAY {d}"),
        None => "THE THREE WEEKS".into(),
    };
    c.text(&TINY, 1, 4, 42, &label, DIM);
    for i in 1..=22u8 {
        let x = 5 + (i as i32 - 1) * 5;
        let nine = i >= NINE_DAYS_FROM;
        let color = match day {
            Some(d) if i == d => WHITE,
            Some(d) if i < d => SLATE,
            _ if nine => PURPLE,
            _ => DARK,
        };
        c.fill_rect(x, 49, 4, 5, color);
        if i == 1 || i == 22 {
            c.hline(x, 55, 4, FAST);
        }
    }

    let begins = f.begins;
    let ends = f.ends;
    c.blit(&sprites::ICON_PLATE, 4, 60);
    c.text(&TINY, 1, 13, 62, "BEGINS", DIM);
    c.text_right(&BODY, 1, 116, 61, &when(begins), WHITE);
    c.blit(&sprites::ICON_PLATE, 4, 72);
    c.text(&TINY, 1, 13, 74, "ENDS", DIM);
    c.text_right(&BODY, 1, 116, 73, &when(ends), WHITE);
    let note = if f.name.contains("observed") {
        "POSTPONED FROM SHABBAT".to_string()
    } else if let (Some(b), Some(e)) = (begins, ends) {
        let mins = (e.utc - b.utc) / 60;
        format!("FAST OF ABOUT {}H {:02}M", mins / 60, mins % 60)
    } else {
        String::new()
    };
    line(c, 85, &note, if f.name.contains("observed") { FAST } else { DIM });
    let big = match (begins, ends) {
        (Some(b), _) if s.now < b.utc => Some(("BEGINS IN", b.utc - s.now)),
        (_, Some(e)) if s.now < e.utc => Some(("ENDS IN", e.utc - s.now)),
        _ => None,
    };
    if let Some((label, secs)) = big {
        c.text(&TINY, 1, 4, 95, label, DIM);
        c.text(&BODY, 1, 4, 102, &countdown(secs), YELLOW);
    }
    c.text_right(&TINY, 1, 116, 95, "EICHA AT NIGHT", DIM);
    c.text_right(&TINY, 1, 116, 103, "NO LEATHER SHOES", DIM);
}
