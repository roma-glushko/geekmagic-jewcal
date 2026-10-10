// Crash accounting in RTC memory (survives resets, not power cuts). Three
// abnormal restarts in a row (exception, watchdog, Rust panic) start safe
// mode: no Rust core, just WiFi and the firmware update page. Without a
// USB-serial chip that page is the only way back short of opening the case.
#pragma once

#include <stddef.h>
#include <stdint.h>

namespace crash {

constexpr uint32_t SAFE_AFTER = 3;

// Count the reset that just happened. Returns true to start in safe mode.
bool begin();
// The firmware has run long enough: forget earlier crashes.
void stable();
// Forget everything (the user asked to leave safe mode).
void clear();
// From jc_panic, just before restarting.
void record_panic(const char *msg, size_t len);

uint32_t count();
// The last panic message or exception dump ("" if none since power-up).
const char *last();

}  // namespace crash
