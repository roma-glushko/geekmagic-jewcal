//! LEDC PWM backlight with day/night brightness.
//!
//! The SmallTV Pro drives the backlight through a P-MOSFET, so on the common
//! board variant (GPIO25) the signal is inverted: duty 0 = full brightness.

use esp_idf_hal::{
    gpio::OutputPin,
    ledc::{config::TimerConfig, LedcChannel, LedcDriver, LedcTimer, LedcTimerDriver, LowSpeed, Resolution},
    units::FromValueType,
};

pub struct Backlight<'d> {
    ch: LedcDriver<'d>,
    max: u32,
    inverted: bool,
    percent: u8,
}

impl<'d> Backlight<'d> {
    pub fn new(
        timer: impl LedcTimer<SpeedMode = LowSpeed> + 'd,
        channel: impl LedcChannel<SpeedMode = LowSpeed> + 'd,
        pin: impl OutputPin + 'd,
        inverted: bool,
    ) -> anyhow::Result<Self> {
        let timer = LedcTimerDriver::new(
            timer,
            &TimerConfig::new().frequency(5.kHz().into()).resolution(Resolution::Bits8),
        )?;
        let ch = LedcDriver::new(channel, timer, pin)?;
        let max = ch.get_max_duty();
        let mut bl = Self { ch, max, inverted, percent: 0 };
        bl.set_percent(100)?;
        Ok(bl)
    }

    /// 0–100. A no-op when the level is unchanged.
    pub fn set_percent(&mut self, percent: u8) -> anyhow::Result<()> {
        let percent = percent.min(100);
        if percent == self.percent {
            return Ok(());
        }
        let on = self.max * percent as u32 / 100;
        let duty = if self.inverted { self.max - on } else { on };
        self.ch.set_duty(duty)?;
        self.percent = percent;
        Ok(())
    }
}
