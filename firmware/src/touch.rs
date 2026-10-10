//! Top touch sensor: GPIO32 = touch pad 9, read through ESP-IDF's legacy
//! touch-pad driver (esp-idf-hal has no safe wrapper for it).
//!
//! On the ESP32 the raw reading *drops* when a finger is on the pad. The
//! baseline is sampled at boot, and a touch is a reading below
//! `baseline * TOUCH_RATIO`. The community-reported raw threshold for this
//! board is about 1450, but it varies per unit, hence the relative threshold.

use esp_idf_hal::delay::FreeRtos;
use esp_idf_svc::sys::{self, esp};
use std::sync::mpsc::Sender;

const PAD: sys::touch_pad_t = sys::touch_pad_t_TOUCH_PAD_NUM9;
const TOUCH_RATIO_PCT: u32 = 80;
const POLL_MS: u32 = 30;
const LONG_PRESS_MS: u32 = 1_500;
/// Holding this long forgets the WiFi network and restarts into setup.
const VERY_LONG_MS: u32 = 8_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Touch {
    Tap,
    LongPress,
    VeryLong,
}

fn read() -> anyhow::Result<u16> {
    let mut v: u16 = 0;
    esp!(unsafe { sys::touch_pad_read_filtered(PAD, &mut v) })?;
    Ok(v)
}

/// Initialise the pad and measure the untouched baseline.
pub fn init() -> anyhow::Result<u16> {
    unsafe {
        esp!(sys::touch_pad_init())?;
        esp!(sys::touch_pad_set_voltage(
            sys::touch_high_volt_t_TOUCH_HVOLT_2V7,
            sys::touch_low_volt_t_TOUCH_LVOLT_0V5,
            sys::touch_volt_atten_t_TOUCH_HVOLT_ATTEN_1V,
        ))?;
        esp!(sys::touch_pad_config(PAD, 0))?;
        esp!(sys::touch_pad_filter_start(10))?;
    }
    FreeRtos::delay_ms(200);
    let mut sum = 0u32;
    for _ in 0..8 {
        sum += read()? as u32;
        FreeRtos::delay_ms(20);
    }
    Ok((sum / 8) as u16)
}

/// Poll forever, reporting taps and long presses. Runs on its own thread.
pub fn run(baseline: u16, tx: Sender<Touch>) {
    let threshold = baseline as u32 * TOUCH_RATIO_PCT / 100;
    log::info!("touch: baseline {baseline}, threshold {threshold}");
    let mut held_ms: u32 = 0;
    let mut long_sent = false;
    let mut very_long_sent = false;
    loop {
        FreeRtos::delay_ms(POLL_MS);
        let pressed = read().is_ok_and(|v| (v as u32) < threshold);
        if pressed {
            held_ms += POLL_MS;
            if held_ms >= LONG_PRESS_MS && !long_sent {
                long_sent = true;
                tx.send(Touch::LongPress).ok();
            }
            if held_ms >= VERY_LONG_MS && !very_long_sent {
                very_long_sent = true;
                tx.send(Touch::VeryLong).ok();
            }
        } else {
            if held_ms >= 2 * POLL_MS && !long_sent {
                tx.send(Touch::Tap).ok();
            }
            held_ms = 0;
            long_sent = false;
            very_long_sent = false;
        }
    }
}
