// Push the Rust canvas to the panel. The ESP8266 cannot keep a second copy
// of the frame to diff against, so each art row's FNV-1a hash is remembered
// (480 bytes) and only rows whose hash changed are sent.

#include <Arduino.h>

#include "display.h"
#include "display_ll.h"
#include "jewcal_core.h"

namespace display {

namespace {

constexpr int ART = 120;  // canvas size; one art pixel = 2x2 panel pixels

uint32_t hashes[ART];
uint8_t lut_be[16][2];
bool lut_ready = false;

uint32_t fnv1a(const uint8_t *p, size_t n) {
  uint32_t h = 2166136261u;
  while (n--) h = (h ^ *p++) * 16777619u;
  return h;
}

}  // namespace

void flush(bool all) {
  if (!lut_ready) {
    uint16_t lut[16];
    jc_lut(lut);
    for (int i = 0; i < 16; i++) {
      lut_be[i][0] = lut[i] >> 8;
      lut_be[i][1] = lut[i] & 0xFF;
    }
    lut_ready = true;
  }
  all = all || stale;
  stale = false;

  uint8_t row[ART];
  uint8_t line[ART * 4];  // one panel row: 240 pixels, 2 bytes each
  bool open = false;      // a RAM write is running from the current row down
  for (int y = 0; y < ART; y++) {
    jc_row(y, row);
    uint32_t h = fnv1a(row, ART);
    if (!all && h == hashes[y]) {
      open = false;
      continue;
    }
    hashes[y] = h;
    if (!open) {
      // To the bottom of the panel: consecutive changed rows just follow.
      window(0, y * 2, 239, 239);
      open = true;
    }
    for (int x = 0; x < ART; x++) {
      const uint8_t *c = lut_be[row[x] & 0x0F];
      uint8_t *o = &line[x * 4];
      o[0] = c[0];
      o[1] = c[1];
      o[2] = c[0];
      o[3] = c[1];
    }
    write(line, sizeof line);
    write(line, sizeof line);
    if ((y & 31) == 31) yield();
  }
}

}  // namespace display
