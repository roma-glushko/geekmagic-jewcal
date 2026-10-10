//! Pixel-art screens for the JewCal display.
//!
//! Everything renders into a 120×120 palette-indexed [`Canvas`] (one art
//! pixel = 2×2 panel pixels on the 240×240 ST7789), using the 16-color
//! Sweetie-16 palette, ASCII-art fonts and sprites decoded at compile time.
//! Pure Rust with no I/O, so the simulator on the host renders exactly what
//! the device shows. `no_std` + `alloc`, so the same code runs on the ESP8266.

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

pub mod canvas;
pub mod font;
pub mod fonts;
pub mod palette;
pub mod scheduler;
pub mod screens;
pub mod sprite;
pub mod sprites;
pub mod text;

pub use canvas::Canvas;
pub use scheduler::Scheduler;
pub use screens::{available, draw, draw_info, draw_message, draw_setup, Ctx, Screen, Status};
