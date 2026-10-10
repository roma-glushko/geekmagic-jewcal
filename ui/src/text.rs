//! Turning Hebcal titles into short, upper-case screen text that fits.

use crate::prelude::*;
use jewcal_model::time::{month_short, CivilDate, Stamp};

use crate::font::Font;

/// Display form of a Hebcal title: `Parashat Noach` → `NOACH`,
/// `Pesach III (CH''M)` → `PESACH III CH"M`, `Rosh Hashana 5788` →
/// `ROSH HASHANA`, `Tish'a B'Av (observed)` → `TISHA B'AV`.
pub fn title(t: &str) -> String {
    let mut s = t.strip_prefix("Parashat ").unwrap_or(t).to_string();
    if let Some((head, tail)) = s.rsplit_once(' ') {
        if head == "Rosh Hashana" && tail.bytes().all(|b| b.is_ascii_digit()) {
            s = head.to_string();
        }
    }
    if s.contains("(Hoshana Raba)") {
        s = "Hoshana Raba".into();
    }
    s = s
        .replace(" (CH''M)", " CH\"M")
        .replace(" (observed)", "")
        .replace("Tish'a B'Av", "Tisha B'Av")
        .replace("Shabbat Mevarchim Chodesh ", "Mevarchim ")
        .replace("Mevarchim Chodesh ", "Mevarchim ");
    if let Some(rest) = s.strip_prefix("Chanukah: ") {
        s = match rest.split(' ').next().and_then(|n| n.parse::<u8>().ok()) {
            Some(n) => format!("Chanukah {n}"),
            None => "Chanukah 8th day".into(),
        };
    }
    s.to_ascii_uppercase()
}

/// Progressive abbreviations, applied in order until the text fits.
const ABBREV: &[(&str, &str)] = &[
    (" & ", "/"),
    ("CHOL HAMOED", "CH\"M"),
    ("SHMINI ATZERET", "SHMINI ATZ."),
    ("SIMCHAT TORAH", "SIMCHAT T."),
    ("SHMINI ATZ./SIMCHAT T.", "SH.ATZERET/S.TORAH"),
    ("ROSH CHODESH", "R\"CH"),
    ("ROSH HASHANA", "R. HASHANA"),
    ("YOM HAATZMA'UT", "ATZMA'UT"),
    ("YOM HAZIKARON", "ZIKARON"),
    ("YOM YERUSHALAYIM", "YOM YERUSH."),
    ("YOM HASHOAH", "HASHOAH"),
    ("YOM HAALIYAH", "ALIYAH DAY"),
    ("HOSHANA RABA", "HOSH. RABA"),
    ("MEVARCHIM", "MEV."),
    ("SHUSHAN ", "SH. "),
    ("TA'ANIT ", "TA'AN. "),
    ("BECHOROT", "BECHOR."),
    ("ASARA B'TEVET", "10 TEVET"),
    ("TZOM GEDALIAH", "TZOM GEDAL."),
    ("SHABBAT", "SHAB."),
    ("PESACH", "PES."),
    ("SUKKOT", "SUK."),
    ("SHAVUOT", "SHAV."),
];

#[derive(Debug, PartialEq, Eq)]
pub struct Fit {
    pub text: String,
    /// The abbreviations were not enough and the text was cut.
    pub truncated: bool,
}

/// Shorten `text` until it is at most `max_w` art pixels wide.
pub fn fit(font: &Font, scale: i32, text: &str, max_w: i32) -> Fit {
    let mut s = text.to_string();
    if font.width(&s, scale) <= max_w {
        return Fit { text: s, truncated: false };
    }
    for (long, short) in ABBREV {
        if s.contains(long) {
            s = s.replace(long, short);
            if font.width(&s, scale) <= max_w {
                return Fit { text: s, truncated: false };
            }
        }
    }
    while !s.is_empty() && font.width(&format!("{s}."), scale) > max_w {
        s.pop();
    }
    let s = s.trim_end().to_string() + ".";
    Fit { text: s, truncated: true }
}

/// Greedy word wrap at spaces and after hyphens, at most `max_lines`; the
/// last line is shortened with [`fit`] if needed.
pub fn wrap(font: &Font, scale: i32, text: &str, max_w: i32, max_lines: usize) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for w in text.split(' ') {
        // Keep "VAYAKHEL-" and "PEKUDEI" as separate breakable parts.
        let mut rest = w;
        while let Some(i) = rest.find('-') {
            words.push(rest[..=i].to_string());
            rest = &rest[i + 1..];
        }
        if !rest.is_empty() {
            words.push(rest.to_string());
        }
    }
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for w in words {
        let candidate = if cur.is_empty() || cur.ends_with('-') { format!("{cur}{w}") } else { format!("{cur} {w}") };
        if font.width(&candidate, scale) <= max_w || cur.is_empty() {
            cur = candidate;
        } else {
            lines.push(core::mem::take(&mut cur));
            cur = w;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.len() > max_lines {
        let tail = lines[max_lines - 1..].join(" ");
        lines.truncate(max_lines - 1);
        lines.push(tail);
    }
    if let Some(last) = lines.last_mut() {
        *last = fit(font, scale, last, max_w).text;
    }
    lines
}

pub fn hm(s: Stamp) -> String {
    let (h, m) = s.hm();
    format!("{h:02}:{m:02}")
}

pub fn opt_hm(s: Option<Stamp>) -> String {
    s.map_or_else(|| "--:--".into(), hm)
}

/// `APR 21`
pub fn date(d: CivilDate) -> String {
    format!("{} {}", month_short(d.m), d.d)
}

/// `WED APR 21`
pub fn weekday_date(d: CivilDate) -> String {
    format!("{} {}", d.weekday().short(), date(d))
}

/// `APR 22-24`, `APR 30-MAY 1`, `APR 22`.
pub fn range(a: CivilDate, b: CivilDate) -> String {
    if a == b {
        date(a)
    } else if a.m == b.m {
        format!("{}-{}", date(a), b.d)
    } else {
        format!("{}-{}", date(a), date(b))
    }
}

/// `WED-FRI`, or `WED` for one day.
pub fn weekdays(a: CivilDate, b: CivilDate) -> String {
    if a == b {
        a.weekday().short().to_string()
    } else {
        format!("{}-{}", a.weekday().short(), b.weekday().short())
    }
}

/// `2D 5H` beyond a day, `5H 22M` beyond an hour, else `22:07`.
pub fn countdown(secs: i64) -> String {
    let secs = secs.max(0);
    let (d, h, m, s) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60, secs % 60);
    if d > 0 {
        format!("{d}D {h}H")
    } else if h > 0 {
        format!("{h}H {m:02}M")
    } else {
        format!("{m}:{s:02}")
    }
}

/// `TODAY`, `TONIGHT`, `TOMORROW`, `IN 12D`.
pub fn days_until(today: CivilDate, first: CivilDate, from_evening: bool) -> String {
    let n = first.days() - today.days();
    match n {
        i64::MIN..=-1 => "NOW".into(),
        0 if from_evening => "TONIGHT".into(),
        0 => "TODAY".into(),
        1 => "TOMORROW".into(),
        _ => format!("IN {n}D"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::{BODY, TINY};

    #[test]
    fn titles() {
        assert_eq!(title("Parashat Vayakhel-Pekudei"), "VAYAKHEL-PEKUDEI");
        assert_eq!(title("Pesach III (CH''M)"), "PESACH III CH\"M");
        assert_eq!(title("Rosh Hashana 5788"), "ROSH HASHANA");
        assert_eq!(title("Rosh Hashana II"), "ROSH HASHANA II");
        assert_eq!(title("Sukkot VII (Hoshana Raba)"), "HOSHANA RABA");
        assert_eq!(title("Tish'a B'Av (observed)"), "TISHA B'AV");
        assert_eq!(title("Chanukah: 3 Candles"), "CHANUKAH 3");
        assert_eq!(title("Mevarchim Chodesh Cheshvan"), "MEVARCHIM CHESHVAN");
    }

    #[test]
    fn fitting() {
        let f = fit(&BODY, 1, "SHMINI ATZERET & SIMCHAT TORAH", 102);
        assert!(!f.truncated, "{f:?}");
        assert!(BODY.width(&f.text, 1) <= 102);
        let w = wrap(&BODY, 1, "ACHREI MOT-KEDOSHIM", 80, 2);
        assert_eq!(w, vec!["ACHREI MOT-", "KEDOSHIM"]);
        assert_eq!(countdown(2 * 86_400 + 5 * 3600 + 59), "2D 5H");
        assert_eq!(countdown(5 * 3600 + 22 * 60), "5H 22M");
        assert_eq!(countdown(22 * 60 + 7), "22:07");
        assert!(TINY.width("WED APR 21", 1) < 50);
    }
}
