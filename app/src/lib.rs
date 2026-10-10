//! Device logic shared by the JewCal firmwares: the SmallTV Pro (ESP32,
//! `firmware/`) and the SmallTV Ultra (ESP8266, `ultra/`).
//!
//! [`settings`] is what the user configures and what it means for Hebcal
//! and the display; [`web`] renders the settings page and applies its form;
//! [`device`] is the SmallTV Ultra's single-threaded main loop.
//! Storage and HTTP serving stay in each firmware (NVS vs EEPROM,
//! `EspHttpServer` vs `ESP8266WebServer`).

#![no_std]

extern crate alloc;
#[cfg(test)]
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

pub mod device;
pub mod settings;
pub mod web;

pub use settings::Settings;
