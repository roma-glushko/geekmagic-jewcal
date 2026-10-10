#include "crash.h"

#include <Arduino.h>
#include <esp8266_peri.h>
#include <user_interface.h>

namespace crash {

namespace {

constexpr uint32_t MAGIC = 0x4C41434A;  // "JCAL"
// RTC user memory offset in 4-byte blocks: the first 128 bytes hold the OTA
// bootloader's command (eboot_command.h).
constexpr uint32_t OFFSET = 32;

struct State {
  uint32_t magic;
  uint32_t crashes;
  uint32_t panicked;
  char last[116];
};
static_assert(sizeof(State) == 128, "RTC record must stay within user memory");

State st;

void save() { ESP.rtcUserMemoryWrite(OFFSET, reinterpret_cast<uint32_t *>(&st), sizeof st); }

}  // namespace

bool begin() {
  ESP.rtcUserMemoryRead(OFFSET, reinterpret_cast<uint32_t *>(&st), sizeof st);
  const uint32_t reason = ESP.getResetInfoPtr()->reason;
  if (st.magic != MAGIC || reason == REASON_DEFAULT_RST || reason == REASON_EXT_SYS_RST) {
    // Power-up: RTC memory holds garbage.
    memset(&st, 0, sizeof st);
    st.magic = MAGIC;
  }
  st.last[sizeof st.last - 1] = 0;
  const bool crashed = reason == REASON_WDT_RST || reason == REASON_EXCEPTION_RST ||
                       reason == REASON_SOFT_WDT_RST || (reason == REASON_SOFT_RESTART && st.panicked);
  if (crashed) {
    st.crashes++;
    if (!st.panicked) {
      // "Fatal exception:3 flag:2 (EXCEPTION) epc1:0x40212345 ...": decode
      // epc1 with addr2line and the jewcal-ultra.elf from the release zip.
      snprintf(st.last, sizeof st.last, "%s", ESP.getResetInfo().c_str());
    }
  } else if (reason == REASON_SOFT_RESTART) {
    // Deliberate: after an update, new settings, or WiFi setup.
    st.crashes = 0;
  }
  st.panicked = 0;
  save();
  return st.crashes >= SAFE_AFTER;
}

void stable() {
  if (st.crashes) {
    st.crashes = 0;
    save();
  }
}

void clear() {
  st.crashes = 0;
  st.last[0] = 0;
  save();
}

void record_panic(const char *msg, size_t len) {
  size_t n = len < sizeof st.last - 1 ? len : sizeof st.last - 1;
  memcpy(st.last, msg, n);
  st.last[n] = 0;
  st.panicked = 1;
  save();
}

uint32_t count() { return st.crashes; }

// The core calls this at the end of every crash report: CPU exceptions, but
// also abort(), panic(), a failed `new`, stack smashing and the software
// watchdog. Those restart as a *soft* reset, which begin() would otherwise
// take for a deliberate restart. It runs in the crash handler, so it stays
// in IRAM and writes RTC memory directly (32-bit words only).
extern "C" void IRAM_ATTR custom_crash_callback(struct rst_info *ri, uint32_t, uint32_t) {
  volatile uint32_t *rtc = RTC_USER_MEM + OFFSET;
  if (rtc[0] != MAGIC || rtc[2]) return;  // no record yet, or jc_panic already wrote one
  union {
    char c[sizeof(State::last)];
    uint32_t w[sizeof(State::last) / 4];
  } m;
  for (uint32_t i = 0; i < sizeof m.w / 4; i++) m.w[i] = 0;
  // "crash: reset 2, exception 28, at 0x4020abcd" (decode with addr2line).
  // No calls out of IRAM: plain loops, and ROM division.
  uint32_t n = 0;
  for (const char *p = "crash: reset "; *p;) m.c[n++] = *p++;
  for (uint32_t v = ri->reason % 1000, div = 100, lead = 1; div; div /= 10) {
    const uint32_t d = v / div % 10;
    if (d || !lead || div == 1) m.c[n++] = '0' + d, lead = 0;
  }
  for (const char *p = ", exception "; *p;) m.c[n++] = *p++;
  for (uint32_t v = ri->exccause % 1000, div = 100, lead = 1; div; div /= 10) {
    const uint32_t d = v / div % 10;
    if (d || !lead || div == 1) m.c[n++] = '0' + d, lead = 0;
  }
  for (const char *p = ", at 0x"; *p;) m.c[n++] = *p++;
  for (int shift = 28; shift >= 0; shift -= 4) m.c[n++] = "0123456789abcdef"[(ri->epc1 >> shift) & 15];
  for (uint32_t i = 0; i < sizeof m.w / 4; i++) rtc[3 + i] = m.w[i];
  rtc[2] = 1;  // panicked: count it as a crash
}

const char *last() { return st.last; }

}  // namespace crash
