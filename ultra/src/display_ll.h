// Low-level panel access shared by display.cpp and flush.cpp.
#pragma once

#include <stddef.h>
#include <stdint.h>

namespace display {

// The panel no longer shows the Rust canvas (something was drawn directly):
// the next flush sends every row.
extern bool stale;

// Set the drawing window (inclusive) and start a RAM write: the pixel data
// that follows (big-endian RGB565) fills it row by row.
void window(uint16_t x0, uint16_t y0, uint16_t x1, uint16_t y1);
void write(const uint8_t *data, size_t len);
// `count` pixels of one color.
void repeat(uint16_t rgb565, uint32_t count);

}  // namespace display
