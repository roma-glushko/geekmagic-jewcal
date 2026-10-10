//! Wall-clock time. The board has no RTC: time comes from SNTP after every
//! boot, and nothing calendar-related is shown until it has synced.

use std::time::{SystemTime, UNIX_EPOCH};

use esp_idf_svc::sntp::EspSntp;

/// 2025-01-01: anything earlier means SNTP has not synced yet.
const VALID_AFTER: i64 = 1_735_689_600;

pub fn start() -> anyhow::Result<EspSntp<'static>> {
    Ok(EspSntp::new_default()?)
}

/// Unix seconds (UTC).
pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

pub fn valid() -> bool {
    now() > VALID_AFTER
}

/// Monotonic milliseconds since boot.
pub fn mono_ms() -> u64 {
    (unsafe { esp_idf_svc::sys::esp_timer_get_time() } / 1000) as u64
}
