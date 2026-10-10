// ST7789 240x240 on the SmallTV Ultra, and the PWM backlight.
#pragma once

#include <stddef.h>
#include <stdint.h>

namespace display {

void begin();

// Push the Rust canvas (120x120 palette indices, scaled 2x): only the rows
// that changed since the last flush, unless `all`.
void flush(bool all = false);

// The next flush redraws everything (after drawing directly on the panel).
void invalidate();

// Direct drawing without the Rust core (installer, safe mode).
void fill(uint16_t rgb565);
// Text in the BODY pixel font at `scale`; returns the width drawn.
int text(int x, int y, const char *s, uint16_t fg, uint16_t bg, int scale);
int text_width(const char *s, int scale);
void text_center(int y, const char *s, uint16_t fg, uint16_t bg, int scale);

// Backlight in percent (0 = off). The pin is active low.
void backlight(uint8_t percent);

constexpr uint16_t rgb(uint8_t r, uint8_t g, uint8_t b) {
  return (uint16_t)(((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3));
}

}  // namespace display
