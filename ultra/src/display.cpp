// ST7789 driver for the SmallTV Ultra: the init sequence of the SmallTV Pro
// firmware (same panel), over the ESP8266's HSPI in mode 3 at 40 MHz.

#include "display.h"

#include <Arduino.h>
#include <SPI.h>

#include "board.h"
#include "display_ll.h"
#include "font_body.h"

namespace display {

namespace {

constexpr uint8_t SWRESET = 0x01, SLPOUT = 0x11, NORON = 0x13, INVON = 0x21, DISPON = 0x29;
constexpr uint8_t CASET = 0x2A, RASET = 0x2B, RAMWR = 0x2C, MADCTL = 0x36, COLMOD = 0x3A;

void cmd(uint8_t c) {
  digitalWrite(PIN_DC, LOW);
  SPI.write(c);
  digitalWrite(PIN_DC, HIGH);
}

void data8(uint8_t d) { SPI.write(d); }

bool find_glyph(char ch, CGlyph &out) {
  if (ch >= 'a' && ch <= 'z') ch -= 'a' - 'A';
  for (uint8_t i = 0; i < FONT_BODY_COUNT; i++) {
    if ((char)pgm_read_byte(&FONT_BODY[i].ch) == ch) {
      memcpy_P(&out, &FONT_BODY[i], sizeof out);
      return true;
    }
  }
  return ch != '?' && find_glyph('?', out);
}

}  // namespace

bool stale = true;

void invalidate() { stale = true; }

void window(uint16_t x0, uint16_t y0, uint16_t x1, uint16_t y1) {
  cmd(CASET);
  data8(x0 >> 8);
  data8(x0);
  data8(x1 >> 8);
  data8(x1);
  cmd(RASET);
  data8(y0 >> 8);
  data8(y0);
  data8(y1 >> 8);
  data8(y1);
  cmd(RAMWR);
}

void write(const uint8_t *data, size_t len) { SPI.writeBytes(data, len); }

void repeat(uint16_t rgb565, uint32_t count) {
  uint8_t px[64];
  for (size_t i = 0; i < sizeof px; i += 2) {
    px[i] = rgb565 >> 8;
    px[i + 1] = rgb565 & 0xFF;
  }
  while (count) {
    uint32_t n = count < sizeof px / 2 ? count : sizeof px / 2;
    SPI.writeBytes(px, n * 2);
    count -= n;
  }
}

void begin() {
  pinMode(PIN_CS, OUTPUT);
  digitalWrite(PIN_CS, LOW);
  pinMode(PIN_DC, OUTPUT);
  digitalWrite(PIN_DC, HIGH);
  // Hold the panel in reset while the SPI clock settles.
  pinMode(PIN_RST, OUTPUT);
  digitalWrite(PIN_RST, LOW);

  // SPI first, then release the reset. With its CS tied low the panel counts
  // every rising clock edge, and mode 3 parks the clock high. After a power-up
  // the clock-polarity bit starts at 0 (SPI.begin() leaves it alone), so
  // switching to mode 3 raises the clock: done after the reset, that edge is
  // a stray bit that shifts every byte that follows, and the panel stays dark.
  // A warm restart kept the bit from the previous run, which hid this.
  SPI.begin();
  SPI.setHwCs(false);
  SPI.beginTransaction(SPISettings(SPI_HZ, MSBFIRST, SPI_MODE3));
  delay(10);
  digitalWrite(PIN_RST, HIGH);
  delay(120);

  cmd(SWRESET);
  delay(150);
  cmd(SLPOUT);
  delay(10);
  cmd(COLMOD);
  data8(0x55);  // 16 bits per pixel
  cmd(MADCTL);
  data8(0x00);
  cmd(INVON);  // this IPS panel needs inversion on
  cmd(NORON);
  delay(10);
  cmd(DISPON);
  delay(10);
}

void fill(uint16_t rgb565) {
  window(0, 0, 239, 239);
  repeat(rgb565, 240UL * 240);
  invalidate();
}

int text_width(const char *s, int scale) {
  int w = 0;
  CGlyph g;
  for (; *s; s++) {
    if (find_glyph(*s, g)) w += (g.w + FONT_BODY_SPACING) * scale;
  }
  return w > 0 ? w - FONT_BODY_SPACING * scale : 0;
}

int text(int x, int y, const char *s, uint16_t fg, uint16_t bg, int scale) {
  if (scale < 1) scale = 1;
  if (scale > 4) scale = 4;
  const int h = FONT_BODY_HEIGHT * scale;
  const int x0 = x;
  uint8_t line[2 * 9 * 4];  // one glyph row: 8 columns + spacing, at scale 4
  CGlyph g;
  for (; *s; s++) {
    if (!find_glyph(*s, g)) continue;
    const int cw = (g.w + FONT_BODY_SPACING) * scale;
    if (x + cw > 240 || y + h > 240 || x < 0 || y < 0) break;
    window(x, y, x + cw - 1, y + h - 1);
    for (int row = 0; row < FONT_BODY_HEIGHT; row++) {
      int o = 0;
      for (int col = 0; col < g.w + FONT_BODY_SPACING; col++) {
        bool ink = col < g.w && (g.rows[row] & (1 << (g.w - 1 - col)));
        uint16_t c = ink ? fg : bg;
        for (int k = 0; k < scale; k++) {
          line[o++] = c >> 8;
          line[o++] = c & 0xFF;
        }
      }
      for (int k = 0; k < scale; k++) SPI.writeBytes(line, o);
    }
    x += cw;
  }
  invalidate();
  return x - x0;
}

void text_center(int y, const char *s, uint16_t fg, uint16_t bg, int scale) {
  int w = text_width(s, scale);
  text((240 - w) / 2, y, s, fg, bg, scale);
}

void backlight(uint8_t percent) {
  static bool ready = false;
  if (!ready) {
    analogWriteRange(255);
    analogWriteFreq(2000);
    ready = true;
  }
  if (percent > 100) percent = 100;
  // Active low: 255 is off.
  analogWrite(PIN_BL, 255 - (uint32_t)percent * 255 / 100);
}

}  // namespace display
