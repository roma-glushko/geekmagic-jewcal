//! Hebcal JSON item → [`Item`] with a [`Kind`] the rest of the model understands.
//!
//! Hebcal titles are stable Sephardic transliterations (`Pesach III (CH''M)`,
//! `Tish'a B'Av`, `Chanukah: 3 Candles`), so a few title patterns complete
//! what `category`/`subcat`/`yomtov` do not say on their own.

use crate::prelude::*;
use crate::time::{parse_hebcal_date, CivilDate, Stamp};
use crate::wire::WireItem;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// "Candle lighting" (Erev Shabbat/YT, or after nightfall between sacred days).
    Candles,
    Havdalah,
    /// A day on which work is forbidden (`yomtov: true`), incl. Yom Kippur.
    YomTov,
    /// Chol HaMoed, incl. Hoshana Raba.
    CholHaMoed,
    /// `Erev …` days.
    Erev,
    /// The fast day itself (minor fasts, Tisha B'Av; not Yom Kippur).
    Fast,
    /// Other `major` days that are not Yom Tov (Purim, Chanukah 8th day…).
    Major,
    Minor,
    Modern,
    /// Shabbat Shekalim / Zachor / Parah / HaChodesh / HaGadol / Shuva / …
    SpecialShabbat,
    /// "Mevarchim Chodesh X" (the Shabbat before Rosh Chodesh).
    Mevarchim,
    /// Chanukah candle lighting for night `n`; `0` is "Chanukah: 8th Day".
    Chanukah(u8),
    FastBegins,
    FastEnds,
    RoshChodesh,
    Parsha,
}

impl Kind {
    /// Worth keeping beyond the detailed window: feeds the agenda and the
    /// "next Yom Tov" line, but not candle times or parsha.
    pub fn is_agenda(self) -> bool {
        matches!(
            self,
            Kind::YomTov
                | Kind::CholHaMoed
                | Kind::Fast
                | Kind::Major
                | Kind::Minor
                | Kind::Modern
                | Kind::RoshChodesh
                | Kind::Chanukah(_)
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub date: CivilDate,
    pub at: Option<Stamp>,
    pub kind: Kind,
    pub title: String,
    /// Kept only where it carries meaning: the occasion of candles/havdalah
    /// ("Pesach I", "Parashat Noach") and the fast named by "Fast begins/ends".
    pub memo: Option<String>,
}

/// Turn one wire item into a model item, or `None` for categories the
/// display does not use (molad, Daf Yomi, Kiddush Levana, Hebrew dates…).
pub fn classify(w: WireItem) -> Option<Item> {
    let (date, at) = parse_hebcal_date(w.date.as_deref()?)?;
    let title = normalize(w.title_orig.or(w.title).unwrap_or_default());
    let sub = w.subcat.as_deref().unwrap_or("");
    let kind = match w.category.as_deref().unwrap_or("") {
        "candles" => Kind::Candles,
        "havdalah" => Kind::Havdalah,
        "parashat" => Kind::Parsha,
        "roshchodesh" => Kind::RoshChodesh,
        "mevarchim" => Kind::Mevarchim,
        "zmanim" if sub == "fast" => {
            if title.starts_with("Fast begins") {
                Kind::FastBegins
            } else if title.starts_with("Fast ends") {
                Kind::FastEnds
            } else {
                return None;
            }
        }
        "holiday" => holiday_kind(&title, sub, w.yomtov == Some(true)),
        _ => return None,
    };
    if kind == Kind::Modern && !is_notable_modern(&title) {
        return None;
    }
    if kind == Kind::Minor && is_obscure_minor(&title) {
        return None;
    }
    let memo = match kind {
        Kind::Candles | Kind::Havdalah | Kind::FastBegins | Kind::FastEnds => w.memo.map(normalize),
        _ => None,
    };
    Some(Item { date, at, kind, title, memo })
}

/// Hebcal writes `Tish’a B’Av` and `(CH’’M)` with curly quotes; the pixel
/// fonts and the title patterns here are ASCII.
pub fn normalize(s: String) -> String {
    if s.is_ascii() {
        return s;
    }
    s.replace(['\u{2018}', '\u{2019}', '\u{02BC}'], "'").replace(['\u{201C}', '\u{201D}'], "\"")
}

/// `mod=on` brings a dozen Israeli civic days (Family Day, Hebrew Language
/// Day…); keep the ones a Jewish calendar display is expected to show.
fn is_notable_modern(title: &str) -> bool {
    const KEEP: [&str; 4] = ["Yom HaShoah", "Yom HaZikaron", "Yom HaAtzma'ut", "Yom Yerushalayim"];
    KEEP.iter().any(|k| title.starts_with(k))
}

/// Minor days that only add noise to a short agenda.
fn is_obscure_minor(title: &str) -> bool {
    const DROP: [&str; 2] = ["Chag HaBanot", "Rosh Hashana LaBehemot"];
    DROP.iter().any(|d| title.starts_with(d))
}

fn holiday_kind(title: &str, sub: &str, yomtov: bool) -> Kind {
    if yomtov {
        return Kind::YomTov;
    }
    if let Some(rest) = title.strip_prefix("Chanukah: ") {
        // "1 Candle", "2 Candles" … "8 Candles", "8th Day".
        let n = rest.split(' ').next().and_then(|s| s.parse::<u8>().ok());
        return Kind::Chanukah(n.unwrap_or(0));
    }
    if title.contains("(CH''M)") || title.contains("Hoshana Raba") {
        return Kind::CholHaMoed;
    }
    if title.starts_with("Erev ") {
        return Kind::Erev;
    }
    if sub == "fast" || title.starts_with("Tish'a B'Av") {
        return Kind::Fast;
    }
    match sub {
        "shabbat" => Kind::SpecialShabbat,
        "modern" => Kind::Modern,
        "minor" => Kind::Minor,
        _ => Kind::Major,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(title: &str, date: &str, cat: &str, sub: Option<&str>, yt: bool) -> WireItem {
        WireItem {
            title: Some(title.into()),
            date: Some(date.into()),
            category: Some(cat.into()),
            subcat: sub.map(Into::into),
            title_orig: None,
            yomtov: yt.then_some(true),
            memo: Some("memo".into()),
        }
    }

    fn kind(title: &str, cat: &str, sub: Option<&str>, yt: bool) -> Option<Kind> {
        classify(wire(title, "2027-04-22", cat, sub, yt)).map(|i| i.kind)
    }

    #[test]
    fn kinds() {
        assert_eq!(kind("Pesach I", "holiday", Some("major"), true), Some(Kind::YomTov));
        assert_eq!(kind("Pesach III (CH''M)", "holiday", Some("major"), false), Some(Kind::CholHaMoed));
        assert_eq!(kind("Sukkot VII (Hoshana Raba)", "holiday", Some("major"), false), Some(Kind::CholHaMoed));
        assert_eq!(kind("Erev Pesach", "holiday", Some("major"), false), Some(Kind::Erev));
        assert_eq!(kind("Erev Tish'a B'Av", "holiday", Some("major"), false), Some(Kind::Erev));
        assert_eq!(kind("Tish'a B'Av (observed)", "holiday", Some("major"), false), Some(Kind::Fast));
        assert_eq!(kind("Tzom Gedaliah", "holiday", Some("fast"), false), Some(Kind::Fast));
        assert_eq!(kind("Chanukah: 3 Candles", "holiday", Some("major"), false), Some(Kind::Chanukah(3)));
        assert_eq!(kind("Chanukah: 8th Day", "holiday", Some("major"), false), Some(Kind::Chanukah(0)));
        assert_eq!(kind("Shabbat HaGadol", "holiday", Some("shabbat"), false), Some(Kind::SpecialShabbat));
        assert_eq!(kind("Yom HaAtzma'ut", "holiday", Some("modern"), false), Some(Kind::Modern));
        assert_eq!(kind("Family Day", "holiday", Some("modern"), false), None);
        assert_eq!(kind("Pesach III (CH\u{2019}\u{2019}M)", "holiday", Some("major"), false), Some(Kind::CholHaMoed));
        assert_eq!(kind("Tish\u{2019}a B\u{2019}Av", "holiday", Some("major"), false), Some(Kind::Fast));
        assert_eq!(kind("Purim", "holiday", Some("major"), false), Some(Kind::Major));
        assert_eq!(kind("Fast begins", "zmanim", Some("fast"), false), Some(Kind::FastBegins));
        assert_eq!(kind("Latest Kiddush Levana", "zmanim", Some("kiddushLevana"), false), None);
        assert_eq!(kind("Molad Cheshvan", "molad", None, false), None);
    }

    #[test]
    fn memo_only_where_meaningful() {
        let c = classify(wire("Candle lighting: 7:23pm", "2027-04-21T19:23:00-04:00", "candles", None, false)).unwrap();
        assert_eq!(c.memo.as_deref(), Some("memo"));
        assert_eq!(c.at.unwrap().hm(), (19, 23));
        let h = classify(wire("Pesach I", "2027-04-22", "holiday", Some("major"), true)).unwrap();
        assert_eq!(h.memo, None);
    }
}
