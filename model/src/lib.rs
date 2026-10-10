//! Jewish calendar model for the JewCal display.
//!
//! Hebcal.com does all halachic computation (candle lighting, havdalah,
//! fasts, Israel/Diaspora schedules, parsha). This crate performs no I/O: it
//! builds request URLs, stream-parses Hebcal's JSON without floats, and turns
//! the flat item list into what the screens need: periods of Shabbat/Yom Tov
//! with their evening-by-evening candle lighting, fasts, and an agenda.
//!
//! It compiles for the host (tests, simulator), the ESP32 (SmallTV Pro) and,
//! without the default `std` feature, the ESP8266 (SmallTV Ultra).

#![no_std]

extern crate alloc;
#[cfg(any(feature = "std", test))]
extern crate std;

/// What `std`'s prelude would provide.
#[allow(unused_imports)]
pub(crate) mod prelude {
    pub use alloc::borrow::ToOwned;
    pub use alloc::boxed::Box;
    pub use alloc::format;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;
}

pub mod calendar;
pub mod classify;
pub mod hebcal;
pub mod hebrew;
pub mod json;
pub mod snapshot;
pub mod time;
pub mod tz;
mod tz_table;
pub mod weather;
pub mod wire;

pub use calendar::{CalendarModel, Evening, EveningKind, Fast, Note, Period, PeriodDay, Span, SpanKind};
pub use classify::{Item, Kind};
pub use snapshot::{Snapshot, State};
pub use time::{CivilDate, LocalTime, Stamp, Weekday};
pub use tz::Tz;
