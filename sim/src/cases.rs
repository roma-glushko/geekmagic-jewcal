//! Cities and the instants the gallery (and the fixture recorder) cover.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum City {
    Nyc,
    Jerusalem,
    London,
    Moscow,
    Helsinki,
}

impl City {
    pub fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.to_ascii_lowercase().as_str() {
            "nyc" | "new-york" | "newyork" => City::Nyc,
            "jerusalem" | "jlm" => City::Jerusalem,
            "london" => City::London,
            "moscow" => City::Moscow,
            "helsinki" => City::Helsinki,
            other => return Err(format!("unknown city {other:?} (nyc, jerusalem, london, moscow, helsinki)")),
        })
    }

    pub fn geonameid(self) -> u32 {
        match self {
            City::Nyc => 5128581,
            City::Jerusalem => 281184,
            City::London => 2643743,
            City::Moscow => 524901,
            City::Helsinki => 658225,
        }
    }

    pub fn tzid(self) -> &'static str {
        match self {
            City::Nyc => "America/New_York",
            City::Jerusalem => "Asia/Jerusalem",
            City::London => "Europe/London",
            City::Moscow => "Europe/Moscow",
            City::Helsinki => "Europe/Helsinki",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            City::Nyc => "nyc",
            City::Jerusalem => "jerusalem",
            City::London => "london",
            City::Moscow => "moscow",
            City::Helsinki => "helsinki",
        }
    }

    /// Calendar years with recorded events fixtures.
    pub fn years(self) -> &'static [i32] {
        match self {
            City::Nyc | City::Jerusalem => &[2024, 2025, 2026, 2027, 2028],
            City::London | City::Moscow | City::Helsinki => &[2027],
        }
    }
}

#[derive(Clone, Debug)]
pub struct Case {
    pub name: String,
    pub city: City,
    /// Local wall-clock time, `YYYY-MM-DDTHH:MM`.
    pub at: String,
}

fn case(name: &str, city: City, at: &str) -> Case {
    Case { name: name.into(), city, at: at.into() }
}

/// Instants chosen to exercise every screen state, in both schedules.
pub fn gallery() -> Vec<Case> {
    use City::*;
    vec![
        case("today-friday", Nyc, "2026-10-09T12:00"),
        case("shabbat-evening", Nyc, "2026-10-09T20:30"),
        case("chanukah-erev-shabbat", Nyc, "2026-12-04T15:00"),
        case("asara-btevet-fast", Nyc, "2026-12-20T11:00"),
        case("purim", Nyc, "2027-03-23T10:00"),
        case("erev-pesach-3day", Nyc, "2027-04-21T12:00"),
        case("pesach-1-day", Nyc, "2027-04-22T13:00"),
        case("pesach-2nd-night", Nyc, "2027-04-22T21:30"),
        case("pesach-2-friday", Nyc, "2027-04-23T13:00"),
        case("chol-hamoed-pesach", Nyc, "2027-04-26T12:00"),
        case("shavuot-fri-sat", Nyc, "2027-06-10T12:00"),
        case("tisha-bav", Nyc, "2027-08-12T14:00"),
        case("erev-rosh-hashana", Nyc, "2027-10-01T12:00"),
        case("rh-shabbat-night", Nyc, "2027-10-02T21:00"),
        case("erev-yom-kippur", Nyc, "2027-10-10T12:00"),
        case("sukkot-chol-hamoed", Nyc, "2027-10-19T12:00"),
        case("tisha-bav-after-shabbat", Nyc, "2025-08-02T15:00"),
        case("asara-btevet-friday", Nyc, "2025-01-10T10:00"),
        case("shabbat-into-pesach", Nyc, "2025-04-12T14:00"),
        case("jlm-today", Jerusalem, "2026-10-09T12:00"),
        case("jlm-erev-pesach", Jerusalem, "2027-04-21T12:00"),
        case("jlm-pesach-chm-friday", Jerusalem, "2027-04-23T13:00"),
        case("jlm-shavuot", Jerusalem, "2027-06-11T10:00"),
        case("rh-thu-sat-2024", Nyc, "2024-10-03T12:00"),
        case("yk-on-shabbat-2024", Nyc, "2024-10-11T19:00"),
        case("rh-countdown", Nyc, "2027-09-28T12:00"),
        case("ten-days-of-return", Nyc, "2027-10-06T12:00"),
        case("omer-day-44", Nyc, "2027-06-05T12:00"),
        case("purim-countdown", Nyc, "2027-03-19T12:00"),
        case("jlm-sukkot", Jerusalem, "2027-10-19T12:00"),
        case("elul", Nyc, "2027-09-14T12:00"),
        case("elul-shabbat", Nyc, "2027-09-18T12:00"),
        case("omer-mid", Nyc, "2027-05-10T12:00"),
        case("omer-lag-baomer-eve", Nyc, "2027-05-24T21:00"),
        case("nine-days", Nyc, "2027-08-09T12:00"),
        case("tisha-bav-eve", Nyc, "2027-08-11T21:00"),
        case("moscow-tammuz-fast", Moscow, "2027-07-22T12:00"),
        case("helsinki-shavuot-no-havdalah", Helsinki, "2027-06-11T12:00"),
    ]
}
