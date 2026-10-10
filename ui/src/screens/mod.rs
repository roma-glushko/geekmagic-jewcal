//! Screens. Each one redraws the whole 120×120 canvas from a [`Snapshot`];
//! the firmware diffs frames and only sends changed pixels to the panel.

use crate::prelude::*;

mod festival;
mod holidays;
mod info;
mod main_screen;
mod month;
mod period;
mod seasons;
mod upcoming;
mod weather;

use jewcal_model::calendar::{Period, SpanKind};
use jewcal_model::snapshot::{Holiday, Season, Snapshot};
use jewcal_model::weather::Weather;

use crate::canvas::{Canvas, W};
use crate::fonts::{BODY, TINY};
use crate::palette::*;
use crate::sprite::Sprite;
use crate::sprites;

pub use info::{draw_info, draw_message, draw_setup};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Screen {
    Main,
    Period,
    Upcoming,
    Month,
    RoshHashana,
    YomKippur,
    Sukkot,
    Chanukah,
    Purim,
    Pesach,
    Shavuot,
    TishaBav,
    Omer,
    Elul,
    Weather,
    Info,
}

impl Screen {
    pub const ALL: [Screen; 16] = [
        Screen::Main,
        Screen::Period,
        Screen::Upcoming,
        Screen::Month,
        Screen::RoshHashana,
        Screen::YomKippur,
        Screen::Sukkot,
        Screen::Chanukah,
        Screen::Purim,
        Screen::Pesach,
        Screen::Shavuot,
        Screen::TishaBav,
        Screen::Omer,
        Screen::Elul,
        Screen::Weather,
        Screen::Info,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Screen::Main => "main",
            Screen::Period => "period",
            Screen::Upcoming => "upcoming",
            Screen::Month => "month",
            Screen::RoshHashana => "rosh-hashana",
            Screen::YomKippur => "yom-kippur",
            Screen::Sukkot => "sukkot",
            Screen::Chanukah => "chanukah",
            Screen::Purim => "purim",
            Screen::Pesach => "pesach",
            Screen::Shavuot => "shavuot",
            Screen::TishaBav => "tisha-bav",
            Screen::Omer => "omer",
            Screen::Elul => "elul",
            Screen::Weather => "weather",
            Screen::Info => "info",
        }
    }

    fn for_holiday(h: Holiday) -> Screen {
        match h {
            Holiday::RoshHashana => Screen::RoshHashana,
            Holiday::YomKippur => Screen::YomKippur,
            Holiday::Sukkot => Screen::Sukkot,
            Holiday::Chanukah => Screen::Chanukah,
            Holiday::Purim => Screen::Purim,
            Holiday::Pesach => Screen::Pesach,
            Holiday::Shavuot => Screen::Shavuot,
            Holiday::TishaBav => Screen::TishaBav,
        }
    }
}

/// Device facts the screens show but the calendar does not know.
#[derive(Clone, Debug, Default)]
pub struct Status {
    /// `http://…/` of the settings page.
    pub url: Option<String>,
    /// The device's mDNS name (`jewcal.local`), when announced.
    pub host: Option<String>,
    pub location: Option<String>,
    pub israel: bool,
    /// When the calendar was last fetched, already formatted.
    pub updated: Option<String>,
    pub error: Option<String>,
}

pub struct Ctx<'a> {
    /// Animation frame counter (~8 per second).
    pub frame: u32,
    pub status: &'a Status,
    /// Latest weather, if any (shown in the header and on its own screen).
    pub weather: Option<&'a Weather>,
}

/// Secondary screens worth showing right now, in rotation order: the
/// holiday screens that are due come first, then the season (Omer, Elul),
/// then the weather.
pub fn available(s: &Snapshot, weather: Option<&Weather>) -> Vec<Screen> {
    let mut v: Vec<Screen> = s
        .holidays
        .iter()
        .filter(|h| h.holiday != Holiday::Chanukah || s.chanukah.is_some())
        .map(|h| Screen::for_holiday(h.holiday))
        .collect();
    match s.season {
        Some(Season::Omer { .. }) => v.push(Screen::Omer),
        Some(Season::Elul { .. }) => v.push(Screen::Elul),
        _ => {}
    }
    if weather.is_some_and(|w| !w.is_stale(s.now)) {
        v.push(Screen::Weather);
    }
    // Info last: it shows the settings page address, so no touch is needed.
    v.extend([Screen::Period, Screen::Upcoming, Screen::Month, Screen::Info]);
    v
}

pub fn draw(screen: Screen, c: &mut Canvas, s: &Snapshot, ctx: &Ctx) {
    match screen {
        Screen::Main => main_screen::draw(c, s, ctx),
        Screen::Period => period::draw(c, s, ctx),
        Screen::Upcoming => upcoming::draw(c, s, ctx),
        Screen::Month => month::draw(c, s, ctx),
        Screen::RoshHashana => holidays::draw_rosh_hashana(c, s, ctx),
        Screen::YomKippur => holidays::draw_yom_kippur(c, s, ctx),
        Screen::Sukkot => holidays::draw_sukkot(c, s, ctx),
        Screen::Chanukah => festival::draw_chanukah(c, s, ctx),
        Screen::Purim => holidays::draw_purim(c, s, ctx),
        Screen::Pesach => holidays::draw_pesach(c, s, ctx),
        Screen::Shavuot => holidays::draw_shavuot(c, s, ctx),
        Screen::TishaBav => holidays::draw_tisha_bav(c, s, ctx),
        Screen::Omer => seasons::draw_omer(c, s, ctx),
        Screen::Elul => seasons::draw_elul(c, s, ctx),
        Screen::Weather => weather::draw(c, s, ctx),
        Screen::Info => draw_info(c, Some(s), ctx),
    }
    if s.stale {
        // A small red notch: the calendar could not be refreshed for a while.
        c.fill_rect(0, 0, 3, 3, RED);
    }
}

// ── Shared pieces ────────────────────────────────────────────────────────────

pub(crate) fn span_color(kind: SpanKind) -> u8 {
    match kind {
        SpanKind::YomTov => YOM_TOV,
        SpanKind::CholHaMoed => CHOL_HAMOED,
        SpanKind::Fast => FAST,
        SpanKind::RoshChodesh => ROSH_CHODESH,
        SpanKind::Modern => MODERN,
        SpanKind::Chanukah => ORANGE,
        SpanKind::Major | SpanKind::Minor => HOLIDAY,
    }
}

pub(crate) fn span_icon(kind: SpanKind) -> &'static Sprite {
    match kind {
        SpanKind::YomTov | SpanKind::CholHaMoed => &sprites::ICON_STAR,
        SpanKind::Fast => &sprites::ICON_PLATE,
        SpanKind::RoshChodesh => &sprites::ICON_MOON,
        SpanKind::Modern => &sprites::ICON_FLAG,
        SpanKind::Chanukah => &sprites::ICON_MENORAH,
        SpanKind::Major | SpanKind::Minor => &sprites::ICON_DOT,
    }
}

/// Hero art for an occasion, plus where its flames go.
pub(crate) fn art_for(title: &str) -> (&'static Sprite, &'static [(i32, i32)]) {
    const NONE: &[(i32, i32)] = &[];
    const MEMORIAL: &[(i32, i32)] = &[sprites::MEMORIAL_FLAME];
    let t = title;
    if t.starts_with("Rosh Hashana") {
        (&sprites::SHOFAR, NONE)
    } else if t.starts_with("Yom Kippur") {
        (&sprites::PRAYING_HANDS, NONE)
    } else if t.starts_with("Yom HaShoah") || t.starts_with("Yom HaZikaron") {
        (&sprites::MEMORIAL, MEMORIAL)
    } else if t.starts_with("Simchat Torah") || t.starts_with("Shmini Atzeret") {
        (&sprites::TORAH, NONE)
    } else if t.starts_with("Sukkot") || t.contains("Hoshana Raba") {
        (&sprites::SUKKAH, NONE)
    } else if t.starts_with("Pesach") {
        (&sprites::MATZAH, NONE)
    } else if t.starts_with("Shavuot") {
        (&sprites::TABLETS, NONE)
    } else if t.starts_with("Purim") || t.starts_with("Shushan Purim") {
        (&sprites::MASK, NONE)
    } else if t.starts_with("Rosh Chodesh") {
        (&sprites::MOON, NONE)
    } else if t.starts_with("Yom HaAtzma") || t.starts_with("Yom Yerushalayim") {
        (&sprites::FLAG, NONE)
    } else if t.starts_with("Shabbat") {
        (&sprites::CANDLES, &sprites::CANDLES_FLAMES)
    } else {
        (&sprites::STAR, NONE)
    }
}

/// Art and accent color for a period: its first Yom Tov, else Shabbat.
pub(crate) fn period_art(p: &Period) -> (&'static Sprite, &'static [(i32, i32)], u8) {
    match p.days.iter().find_map(|d| d.yom_tov.as_deref()) {
        Some(yt) => {
            let (s, f) = art_for(yt);
            (s, f, YOM_TOV)
        }
        None => {
            let (s, f) = art_for("Shabbat");
            (s, f, SHABBAT)
        }
    }
}

/// Draw a sprite with flickering flames at the given offsets.
pub(crate) fn draw_art(c: &mut Canvas, art: &Sprite, flames: &[(i32, i32)], x: i32, y: i32, frame: u32) {
    c.blit(art, x, y);
    for (i, (fx, fy)) in flames.iter().enumerate() {
        let f = ((frame / 2) as usize + i * 2) % sprites::FLAME.len();
        c.blit(&sprites::FLAME[f], x + fx, y + fy);
    }
}

/// Title bar shared by the secondary screens: label left, extra right.
pub(crate) fn title_bar(c: &mut Canvas, label: &str, right: &str, color: u8) {
    c.fill_rect(0, 0, W, 11, PANEL);
    c.hline(0, 11, W, color);
    c.text(&TINY, 1, 3, 3, label, color);
    c.text_right(&TINY, 1, W - 3, 3, right, DIM);
}

/// Centered BODY text that is shortened to fit `max_w`.
pub(crate) fn body_center_fit(c: &mut Canvas, cx: i32, y: i32, text: &str, max_w: i32, color: u8) {
    let f = crate::text::fit(&BODY, 1, text, max_w);
    c.text_center(&BODY, 1, cx, y, &f.text, color);
}
