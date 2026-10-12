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

/// Stable sort by key, in place, with no scratch memory.
///
/// Use it instead of `slice::sort_by_key`: from 21 elements on, core's
/// driftsort keeps a 4 KB scratch buffer on the stack, as large as the
/// SmallTV Ultra's whole loop stack. On 1 Cheshvan 5787 the fetch window
/// shrank to ~60 items, the sort started using that buffer, and every
/// calendar build overflowed the stack (reset 253). Hebcal's answers arrive
/// nearly sorted, so insertion sort is close to linear on them.
pub(crate) fn sort_by_key<T, K: Ord>(v: &mut [T], mut key: impl FnMut(&T) -> K) {
    for i in 1..v.len() {
        let mut j = i;
        while j > 0 && key(&v[j - 1]) > key(&v[j]) {
            v.swap(j - 1, j);
            j -= 1;
        }
    }
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

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn sort_by_key_matches_the_stable_sort() {
        let mut rng = 0x2545_f491_u32;
        for len in [0, 1, 2, 20, 21, 60, 64, 65, 200] {
            // Few distinct keys, so equal keys must keep their order.
            let v: Vec<(u32, usize)> = (0..len)
                .map(|i| {
                    rng ^= rng << 13;
                    rng ^= rng >> 17;
                    rng ^= rng << 5;
                    (rng % 7, i)
                })
                .collect();
            let mut ours = v.clone();
            super::sort_by_key(&mut ours, |p| p.0);
            let mut core = v;
            core.sort_by_key(|p| p.0);
            assert_eq!(ours, core, "len {len}");
        }
    }
}
