//! Which screen is on: the main screen by default; every `cycle` the
//! secondary screens are shown once each for `dwell`, then back to main.
//! A tap advances immediately; a long press shows the info screen.

use crate::prelude::*;
use crate::screens::Screen;

#[derive(Debug)]
pub struct Scheduler {
    cycle_ms: u64,
    dwell_ms: u64,
    current: Screen,
    queue: Vec<Screen>,
    until_ms: u64,
    next_cycle_ms: u64,
}

impl Scheduler {
    pub fn new(cycle_min: u32, dwell_s: u32, now_ms: u64) -> Self {
        let cycle_ms = cycle_min.max(1) as u64 * 60_000;
        Self {
            cycle_ms,
            dwell_ms: dwell_s.max(3) as u64 * 1000,
            current: Screen::Main,
            queue: Vec::new(),
            until_ms: 0,
            next_cycle_ms: now_ms + cycle_ms,
        }
    }

    pub fn set_timing(&mut self, cycle_min: u32, dwell_s: u32, now_ms: u64) {
        self.cycle_ms = cycle_min.max(1) as u64 * 60_000;
        self.dwell_ms = dwell_s.max(3) as u64 * 1000;
        self.next_cycle_ms = self.next_cycle_ms.min(now_ms + self.cycle_ms);
    }

    pub fn current(&self) -> Screen {
        self.current
    }

    /// Advance the clock; `available` lists the secondary screens that are
    /// relevant now, in order.
    pub fn tick(&mut self, now_ms: u64, available: &[Screen]) -> Screen {
        if self.current == Screen::Main {
            if now_ms >= self.next_cycle_ms && !available.is_empty() {
                self.queue = available.to_vec();
                self.advance(now_ms);
            }
        } else if now_ms >= self.until_ms {
            self.advance(now_ms);
        }
        self.current
    }

    pub fn tap(&mut self, now_ms: u64, available: &[Screen]) -> Screen {
        if self.current == Screen::Main {
            self.queue = available.to_vec();
        }
        self.advance(now_ms);
        self.current
    }

    pub fn long_press(&mut self, now_ms: u64) -> Screen {
        self.queue.clear();
        self.current = Screen::Info;
        self.until_ms = now_ms + self.dwell_ms * 2;
        self.current
    }

    fn advance(&mut self, now_ms: u64) {
        if self.queue.is_empty() {
            self.current = Screen::Main;
            self.next_cycle_ms = now_ms + self.cycle_ms;
        } else {
            self.current = self.queue.remove(0);
            self.until_ms = now_ms + self.dwell_ms;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_then_returns_to_main() {
        let avail = [Screen::Period, Screen::Upcoming];
        let mut s = Scheduler::new(3, 10, 0);
        assert_eq!(s.tick(60_000, &avail), Screen::Main);
        assert_eq!(s.tick(180_000, &avail), Screen::Period);
        assert_eq!(s.tick(185_000, &avail), Screen::Period);
        assert_eq!(s.tick(190_000, &avail), Screen::Upcoming);
        assert_eq!(s.tick(200_000, &avail), Screen::Main);
        assert_eq!(s.tick(379_000, &avail), Screen::Main);
        assert_eq!(s.tick(380_000, &avail), Screen::Period);
    }

    #[test]
    fn tap_and_long_press() {
        let avail = [Screen::Period, Screen::Month];
        let mut s = Scheduler::new(3, 10, 0);
        assert_eq!(s.tap(1_000, &avail), Screen::Period);
        assert_eq!(s.tap(2_000, &avail), Screen::Month);
        assert_eq!(s.tap(3_000, &avail), Screen::Main);
        assert_eq!(s.long_press(4_000), Screen::Info);
        assert_eq!(s.tick(30_000, &avail), Screen::Main);
    }
}
